use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct FlyTomlScanner;

#[async_trait]
impl Scanner for FlyTomlScanner {
    fn id(&self) -> &str {
        "fly_toml"
    }

    fn name(&self) -> &str {
        "Fly.io Scanner"
    }

    fn description(&self) -> &str {
        "Detects fly.toml — app name, region, services, VM config"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        let pattern = context.dir.join("**/fly.toml");
        for entry in glob(&pattern.to_string_lossy())?.flatten() {
            if context.is_excluded(&entry) || !entry.is_file() {
                continue;
            }
            if let Some(r) = parse_fly_toml(&entry, &context.dir) {
                resources.push(r);
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
            resolution: None,
        })
    }
}

fn parse_fly_toml(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let (app, region, services, has_http_service) = summarise(&content);

    // Require at least an app name to avoid false positives on unrelated TOML files
    // that happen to be named fly.toml in test fixtures or tutorials.
    let app = app?;

    let mut extra = HashMap::new();
    extra.insert("app".to_string(), serde_json::Value::String(app.clone()));
    if let Some(r) = &region {
        extra.insert(
            "primary_region".to_string(),
            serde_json::Value::String(r.clone()),
        );
    }
    extra.insert(
        "services".to_string(),
        serde_json::Value::Number(serde_json::Number::from(services)),
    );
    if has_http_service {
        extra.insert("http_service".to_string(), serde_json::Value::Bool(true));
    }

    Some(Resource {
        resource_type: "fly_app".to_string(),
        name: Some(app),
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "fly_toml".to_string(),
        estimated_cost: None,
        extra,
    })
}

fn summarise(content: &str) -> (Option<String>, Option<String>, usize, bool) {
    let mut app: Option<String> = None;
    let mut region: Option<String> = None;
    let mut services: usize = 0;
    let mut has_http_service = false;

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[services]]" {
            services += 1;
        }
        if line == "[http_service]" {
            has_http_service = true;
        }
        if let Some(val) = extract_string_value(line, "app") {
            app = Some(val);
        }
        if let Some(val) = extract_string_value(line, "primary_region") {
            region = Some(val);
        }
    }

    (app, region, services, has_http_service)
}

fn extract_string_value(line: &str, key: &str) -> Option<String> {
    let prefix = format!("{} =", key);
    if !line.starts_with(&prefix) {
        return None;
    }
    let rest = line[prefix.len()..].trim();
    let val = rest.trim_matches('"').trim_matches('\'');
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarise_extracts_app_and_region() {
        let toml = r#"
app = "my-app"
primary_region = "fra"

[http_service]
  internal_port = 8080

[[vm]]
  cpu_kind = "shared"
  cpus = 1
  memory_mb = 256
"#;
        let (app, region, services, http) = summarise(toml);
        assert_eq!(app, Some("my-app".to_string()));
        assert_eq!(region, Some("fra".to_string()));
        assert_eq!(services, 0);
        assert!(http);
    }

    #[test]
    fn summarise_counts_services_sections() {
        let toml = r#"
app = "worker-app"

[[services]]
  internal_port = 8080

[[services]]
  internal_port = 9090
"#;
        let (_, _, services, http) = summarise(toml);
        assert_eq!(services, 2);
        assert!(!http);
    }

    #[test]
    fn no_app_returns_none() {
        let toml = "primary_region = \"iad\"\n";
        let result = parse_fly_toml(
            std::path::Path::new("/nonexistent/fly.toml"),
            std::path::Path::new("/nonexistent"),
        );
        assert!(result.is_none());

        let (app, _, _, _) = summarise(toml);
        assert!(app.is_none());
    }
}
