use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct NginxScanner;

#[async_trait]
impl Scanner for NginxScanner {
    fn id(&self) -> &str {
        "nginx"
    }

    fn name(&self) -> &str {
        "nginx Scanner"
    }

    fn description(&self) -> &str {
        "Detects nginx.conf and site configs — server blocks, upstreams, proxies"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        // nginx configs live under a handful of conventional paths. We scan
        // those narrowly rather than every *.conf in the tree — lots of
        // unrelated tools use the .conf extension.
        let patterns = [
            "**/nginx.conf",
            "**/nginx/*.conf",
            "**/sites-available/*",
            "**/sites-enabled/*",
            "**/conf.d/*.conf",
        ];

        for pattern in &patterns {
            let full_pattern = context.dir.join(pattern);
            for entry in glob(&full_pattern.to_string_lossy())?.flatten() {
                if context.is_excluded(&entry) || !entry.is_file() {
                    continue;
                }
                if let Some(r) = parse_nginx_file(&entry, &context.dir) {
                    resources.push(r);
                }
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
            resolution: None,
        })
    }
}

fn parse_nginx_file(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;
    // Cheap fingerprint — avoid claiming every `.conf` in sites-enabled as
    // nginx when it happens to be something else (e.g. a PHP-FPM pool file
    // that also lives under /etc/php/.../conf.d/).
    if !looks_like_nginx(&content) {
        return None;
    }

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let (servers, upstreams, server_names, has_proxy, has_ssl) = summarise(&content);

    let mut extra = HashMap::new();
    extra.insert(
        "server_blocks".to_string(),
        serde_json::Value::Number(serde_json::Number::from(servers)),
    );
    extra.insert(
        "upstreams".to_string(),
        serde_json::Value::Number(serde_json::Number::from(upstreams)),
    );
    if !server_names.is_empty() {
        extra.insert(
            "server_names".to_string(),
            serde_json::Value::Array(
                server_names
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if has_proxy {
        extra.insert("proxy_pass".to_string(), serde_json::Value::Bool(true));
    }
    if has_ssl {
        extra.insert("ssl".to_string(), serde_json::Value::Bool(true));
    }

    Some(Resource {
        resource_type: "nginx_config".to_string(),
        name: None,
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "nginx".to_string(),
        estimated_cost: None,
        extra,
    })
}

fn looks_like_nginx(content: &str) -> bool {
    // At least one of the two directives that only make sense in nginx —
    // `server {` or `upstream <name> {`. The `http {` and `events {`
    // wrappers are common enough that matching them alone produces false
    // positives in other tools' configs.
    content.lines().any(|l| {
        let t = l.trim();
        (t.starts_with("server ") && t.ends_with('{')) || t.starts_with("upstream ")
    }) || content.contains("proxy_pass")
}

fn summarise(content: &str) -> (usize, usize, Vec<String>, bool, bool) {
    let mut servers = 0;
    let mut upstreams = 0;
    let mut server_names: Vec<String> = Vec::new();
    let mut has_proxy = false;
    let mut has_ssl = false;

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("server ") && line.ends_with('{') || line == "server {" {
            servers += 1;
        }
        if line.starts_with("upstream ") {
            upstreams += 1;
        }
        if let Some(rest) = line.strip_prefix("server_name ") {
            for name in rest.trim_end_matches(';').split_whitespace() {
                if !server_names.contains(&name.to_string()) {
                    server_names.push(name.to_string());
                }
            }
        }
        if line.starts_with("proxy_pass ") {
            has_proxy = true;
        }
        if line.starts_with("ssl_certificate ")
            || line.contains(" ssl")
            || line.contains("listen 443")
        {
            has_ssl = true;
        }
    }

    (servers, upstreams, server_names, has_proxy, has_ssl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_nginx_accepts_server_block() {
        assert!(looks_like_nginx("server {\n    listen 80;\n}\n"));
    }

    #[test]
    fn looks_like_nginx_accepts_upstream_block() {
        assert!(looks_like_nginx(
            "upstream api {\n    server 10.0.0.1:3000;\n}\n"
        ));
    }

    #[test]
    fn looks_like_nginx_rejects_unrelated_conf() {
        // A PHP-FPM pool lookalike — lots of `=` + section headers but no
        // nginx-specific keywords. Must not be classified as nginx.
        let php = "[www]\nuser = www-data\npm = dynamic\n";
        assert!(!looks_like_nginx(php));
    }

    #[test]
    fn summarise_counts_blocks_and_extracts_names() {
        let conf = r#"
upstream api_pool {
    server 10.0.0.1:3000;
    server 10.0.0.2:3000;
}

server {
    listen 80;
    server_name example.com www.example.com;
    location / {
        proxy_pass http://api_pool;
    }
}

server {
    listen 443 ssl;
    server_name secure.example.com;
    ssl_certificate /etc/ssl/cert.pem;
}
"#;
        let (servers, upstreams, names, proxy, ssl) = summarise(conf);
        assert_eq!(servers, 2);
        assert_eq!(upstreams, 1);
        assert!(names.contains(&"example.com".to_string()));
        assert!(names.contains(&"secure.example.com".to_string()));
        assert!(proxy);
        assert!(ssl);
    }

    #[test]
    fn summarise_ignores_comments_and_blanks() {
        let conf = "# server { }\n\n# upstream foo { }\n";
        let (servers, upstreams, _, _, _) = summarise(conf);
        assert_eq!(servers, 0);
        assert_eq!(upstreams, 0);
    }
}
