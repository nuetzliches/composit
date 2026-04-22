use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;
use serde_yaml::Value;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct RenderYamlScanner;

#[async_trait]
impl Scanner for RenderYamlScanner {
    fn id(&self) -> &str {
        "render_yaml"
    }

    fn name(&self) -> &str {
        "Render.com Scanner"
    }

    fn description(&self) -> &str {
        "Detects render.yaml — services, types (web/worker/cron), plans"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        let pattern = context.dir.join("**/render.yaml");
        for entry in glob(&pattern.to_string_lossy())?.flatten() {
            if context.is_excluded(&entry) || !entry.is_file() {
                continue;
            }
            resources.extend(parse_render_yaml(&entry, &context.dir));
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}

fn parse_render_yaml(path: &Path, base_dir: &Path) -> Vec<Resource> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let doc: Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let services = match doc.get("services").and_then(|s| s.as_sequence()) {
        Some(s) => s,
        None => return vec![],
    };

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let display_path = format!("./{}", rel);

    services
        .iter()
        .filter_map(|svc| parse_service(svc, &display_path))
        .collect()
}

fn parse_service(svc: &Value, file_path: &str) -> Option<Resource> {
    let name = svc
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string)?;

    let svc_type = svc
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("web")
        .to_string();

    let mut extra = HashMap::new();
    extra.insert(
        "service_type".to_string(),
        serde_json::Value::String(svc_type.clone()),
    );

    if let Some(env) = svc.get("env").and_then(|v| v.as_str()) {
        extra.insert(
            "env".to_string(),
            serde_json::Value::String(env.to_string()),
        );
    }
    if let Some(plan) = svc.get("plan").and_then(|v| v.as_str()) {
        extra.insert(
            "plan".to_string(),
            serde_json::Value::String(plan.to_string()),
        );
    }
    if let Some(region) = svc.get("region").and_then(|v| v.as_str()) {
        extra.insert(
            "region".to_string(),
            serde_json::Value::String(region.to_string()),
        );
    }

    Some(Resource {
        resource_type: "render_service".to_string(),
        name: Some(name),
        path: Some(file_path.to_string()),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "render_yaml".to_string(),
        estimated_cost: None,
        extra,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_service_extracts_name_and_type() {
        let yaml: Value = serde_yaml::from_str(
            r#"
name: api
type: web
env: docker
plan: starter
"#,
        )
        .unwrap();
        let r = parse_service(&yaml, "./render.yaml").unwrap();
        assert_eq!(r.name, Some("api".to_string()));
        assert_eq!(r.extra["service_type"], serde_json::Value::String("web".to_string()));
        assert_eq!(r.extra["plan"], serde_json::Value::String("starter".to_string()));
    }

    #[test]
    fn parse_service_defaults_type_to_web() {
        let yaml: Value = serde_yaml::from_str("name: frontend\n").unwrap();
        let r = parse_service(&yaml, "./render.yaml").unwrap();
        assert_eq!(r.extra["service_type"], serde_json::Value::String("web".to_string()));
    }

    #[test]
    fn parse_service_none_without_name() {
        let yaml: Value = serde_yaml::from_str("type: worker\n").unwrap();
        assert!(parse_service(&yaml, "./render.yaml").is_none());
    }
}
