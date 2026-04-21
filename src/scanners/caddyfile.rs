use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct CaddyfileScanner;

#[async_trait]
impl Scanner for CaddyfileScanner {
    fn id(&self) -> &str {
        "caddyfile"
    }

    fn name(&self) -> &str {
        "Caddyfile Scanner"
    }

    fn description(&self) -> &str {
        "Detects Caddyfile reverse proxy configurations and site blocks"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        let pattern = context.dir.join("**/Caddyfile*");
        for path in glob(&pattern.to_string_lossy())?.flatten() {
            if context.is_excluded(&path) {
                continue;
            }
            let rel_path = path
                .strip_prefix(&context.dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let display_path = format!("./{}", rel_path);

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!(
                        "warning: caddyfile scanner could not read {}: {}",
                        path.display(),
                        e
                    );
                    continue;
                }
            };

            let sites = parse_site_blocks(&content);
            let site_count = sites.len();

            // Caddyfile-level resource
            let mut extra = HashMap::new();
            extra.insert(
                "sites".to_string(),
                serde_json::Value::Number(serde_json::Number::from(site_count)),
            );

            let domains: Vec<String> = sites.iter().map(|s| s.address.clone()).collect();
            if !domains.is_empty() {
                extra.insert(
                    "domains".to_string(),
                    serde_json::Value::Array(
                        domains
                            .iter()
                            .map(|d| serde_json::Value::String(d.clone()))
                            .collect(),
                    ),
                );
            }

            resources.push(Resource {
                resource_type: "caddyfile".to_string(),
                name: None,
                path: Some(display_path.clone()),
                provider: None,
                created: None,
                created_by: None,
                detected_by: "caddyfile".to_string(),
                estimated_cost: None,
                extra,
            });

            // Individual site resources
            for site in &sites {
                let mut site_extra = HashMap::new();
                site_extra.insert(
                    "caddyfile".to_string(),
                    serde_json::Value::String(display_path.clone()),
                );
                if let Some(ref upstream) = site.reverse_proxy {
                    site_extra.insert(
                        "reverse_proxy".to_string(),
                        serde_json::Value::String(upstream.clone()),
                    );
                }
                if site.file_server {
                    site_extra.insert("file_server".to_string(), serde_json::Value::Bool(true));
                }
                if let Some(ref tls) = site.tls {
                    site_extra.insert("tls".to_string(), serde_json::Value::String(tls.clone()));
                }

                let directives: Vec<String> = site.directives.clone();
                if !directives.is_empty() {
                    site_extra.insert(
                        "directives".to_string(),
                        serde_json::Value::Array(
                            directives
                                .into_iter()
                                .map(serde_json::Value::String)
                                .collect(),
                        ),
                    );
                }

                resources.push(Resource {
                    resource_type: "caddy_site".to_string(),
                    name: Some(site.address.clone()),
                    path: Some(display_path.clone()),
                    provider: None,
                    created: None,
                    created_by: None,
                    detected_by: "caddyfile".to_string(),
                    estimated_cost: None,
                    extra: site_extra,
                });
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}

struct SiteBlock {
    address: String,
    reverse_proxy: Option<String>,
    file_server: bool,
    tls: Option<String>,
    directives: Vec<String>,
}

/// Parse Caddyfile content into site blocks.
/// Handles the common Caddyfile format: `address { directives }`.
/// Skips global options blocks (bare `{ ... }` without address).
fn parse_site_blocks(content: &str) -> Vec<SiteBlock> {
    let mut sites = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }

        // Global options block: starts with `{` on its own line (no address before it)
        if trimmed == "{" {
            // Skip until matching close brace
            let mut depth = 1;
            i += 1;
            while i < lines.len() && depth > 0 {
                let t = lines[i].trim();
                depth += t.chars().filter(|&c| c == '{').count();
                depth -= t.chars().filter(|&c| c == '}').count();
                i += 1;
            }
            continue;
        }

        // Site block: `address {` or `address` on one line then `{` on next
        if trimmed.ends_with('{') || (i + 1 < lines.len() && lines[i + 1].trim() == "{") {
            let address_line = if trimmed.ends_with('{') {
                trimmed.trim_end_matches('{').trim()
            } else {
                trimmed
            };

            // Skip if no address (shouldn't happen after global block check, but be safe)
            if address_line.is_empty() {
                i += 1;
                continue;
            }

            let address = address_line.to_string();

            // Find the opening brace if it's on the next line
            if !trimmed.ends_with('{') {
                i += 1; // skip to the `{` line
            }

            // Collect block content
            let mut depth = 1;
            i += 1;
            let mut block_lines = Vec::new();
            while i < lines.len() && depth > 0 {
                let t = lines[i].trim();
                depth += t.chars().filter(|&c| c == '{').count();
                depth -= t.chars().filter(|&c| c == '}').count();
                if depth > 0 {
                    block_lines.push(t);
                }
                i += 1;
            }

            let site = parse_site_content(&address, &block_lines);
            sites.push(site);
        } else {
            i += 1;
        }
    }

    sites
}

fn parse_site_content(address: &str, lines: &[&str]) -> SiteBlock {
    let mut reverse_proxy = None;
    let mut file_server = false;
    let mut tls = None;
    let mut directives = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with("reverse_proxy ") {
            let upstream = trimmed
                .strip_prefix("reverse_proxy ")
                .unwrap_or("")
                .split('{')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if reverse_proxy.is_none() {
                reverse_proxy = Some(upstream);
            }
        } else if trimmed == "file_server" || trimmed.starts_with("file_server ") {
            file_server = true;
        } else if trimmed.starts_with("tls ") {
            tls = Some(
                trimmed
                    .strip_prefix("tls ")
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            );
        }

        // Collect unique top-level directive names
        let directive = trimmed.split_whitespace().next().unwrap_or("");
        if !directive.is_empty()
            && !directive.starts_with('#')
            && !directive.starts_with('}')
            && !directives.contains(&directive.to_string())
        {
            directives.push(directive.to_string());
        }
    }

    SiteBlock {
        address: address.to_string(),
        reverse_proxy,
        file_server,
        tls,
        directives,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_reverse_proxy() {
        let sites = parse_site_blocks(
            r#"
example.com {
    reverse_proxy localhost:8080
}
"#,
        );

        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].address, "example.com");
        assert_eq!(sites[0].reverse_proxy.as_deref(), Some("localhost:8080"));
    }

    #[test]
    fn test_parse_multiple_sites() {
        let sites = parse_site_blocks(
            r#"
api.example.com {
    reverse_proxy api:3000
}

app.example.com {
    file_server
    root * /var/www
}
"#,
        );

        assert_eq!(sites.len(), 2);
        assert_eq!(sites[0].address, "api.example.com");
        assert_eq!(sites[0].reverse_proxy.as_deref(), Some("api:3000"));
        assert_eq!(sites[1].address, "app.example.com");
        assert!(sites[1].file_server);
    }

    #[test]
    fn test_skip_global_options() {
        let sites = parse_site_blocks(
            r#"
{
    email admin@example.com
}

example.com {
    reverse_proxy app:8080
}
"#,
        );

        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].address, "example.com");
    }

    #[test]
    fn test_parse_tls_directive() {
        let sites = parse_site_blocks(
            r#"
{$DOMAIN} {
    tls internal
    reverse_proxy upstream:9000
}
"#,
        );

        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].address, "{$DOMAIN}");
        assert_eq!(sites[0].tls.as_deref(), Some("internal"));
    }

    #[test]
    fn test_env_var_addresses() {
        let sites = parse_site_blocks(
            r#"
{$AUTH_DOMAIN:auth.example.com} {
    tls {$TLS_MODE}
    reverse_proxy server:9000
}

{$API_DOMAIN:api.example.com} {
    reverse_proxy api:3000
}
"#,
        );

        assert_eq!(sites.len(), 2);
        assert_eq!(sites[0].address, "{$AUTH_DOMAIN:auth.example.com}");
        assert_eq!(sites[1].address, "{$API_DOMAIN:api.example.com}");
    }

    #[test]
    fn test_empty_caddyfile() {
        let sites = parse_site_blocks("");
        assert_eq!(sites.len(), 0);

        let sites = parse_site_blocks("# Just a comment\n");
        assert_eq!(sites.len(), 0);
    }
}
