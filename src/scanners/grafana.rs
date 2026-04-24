use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct GrafanaScanner;

#[async_trait]
impl Scanner for GrafanaScanner {
    fn id(&self) -> &str {
        "grafana"
    }

    fn name(&self) -> &str {
        "Grafana Scanner"
    }

    fn description(&self) -> &str {
        "Detects Grafana dashboards (JSON) and provisioning configs (datasources, dashboard providers)"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        // Dashboard JSON lives anywhere — identification is shape-based, not
        // path-based. A real Grafana dashboard always has `schemaVersion`
        // plus either `panels` (top-level) or `rows[].panels` (legacy).
        let dash_pattern = context.dir.join("**/*.json");
        for entry in glob(&dash_pattern.to_string_lossy())?.flatten() {
            if context.is_excluded(&entry) || !entry.is_file() {
                continue;
            }
            if let Some(r) = parse_dashboard(&entry, &context.dir) {
                resources.push(r);
            }
        }

        // Provisioning YAML is path-anchored: Grafana only reads it from
        // directories named `provisioning/{dashboards,datasources,…}/*.yaml`.
        // Scanning every YAML would re-detect docker-compose, K8s, etc.
        for pattern in &[
            "**/provisioning/datasources/*.yaml",
            "**/provisioning/datasources/*.yml",
            "**/provisioning/dashboards/*.yaml",
            "**/provisioning/dashboards/*.yml",
        ] {
            let full = context.dir.join(pattern);
            for entry in glob(&full.to_string_lossy())?.flatten() {
                if context.is_excluded(&entry) || !entry.is_file() {
                    continue;
                }
                resources.extend(parse_provisioning(&entry, &context.dir));
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
            resolution: None,
        })
    }
}

fn parse_dashboard(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;
    let doc: JsonValue = serde_json::from_str(&content).ok()?;
    // `schemaVersion` is the load-bearing signal — it's a Grafana-only key
    // and non-empty in every exported dashboard.
    doc.get("schemaVersion")?;
    let panels = doc.get("panels").and_then(|v| v.as_array());
    if panels.is_none() && doc.get("rows").and_then(|v| v.as_array()).is_none() {
        return None;
    }

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let title = doc
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("dashboard")
        .to_string();
    let uid = doc
        .get("uid")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let schema_version = doc
        .get("schemaVersion")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let panel_count = panels.map(|p| p.len()).unwrap_or(0);

    let mut extra = HashMap::new();
    extra.insert(
        "schema_version".to_string(),
        serde_json::Value::Number(serde_json::Number::from(schema_version)),
    );
    extra.insert(
        "panels".to_string(),
        serde_json::Value::Number(serde_json::Number::from(panel_count)),
    );
    if let Some(u) = uid {
        extra.insert("uid".to_string(), serde_json::Value::String(u));
    }

    Some(Resource {
        resource_type: "grafana_dashboard".to_string(),
        name: Some(title),
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "grafana".to_string(),
        estimated_cost: None,
        extra,
    })
}

fn parse_provisioning(path: &Path, base_dir: &Path) -> Vec<Resource> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return vec![];
    };
    let Ok(doc) = serde_yaml::from_str::<YamlValue>(&content) else {
        return vec![];
    };
    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let display = format!("./{}", rel);

    let mut out = Vec::new();

    if let Some(entries) = doc.get("datasources").and_then(|v| v.as_sequence()) {
        for ds in entries {
            let name = ds
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed")
                .to_string();
            let ds_type = ds
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let url = ds
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut extra = HashMap::new();
            extra.insert("ds_type".to_string(), serde_json::Value::String(ds_type));
            if let Some(u) = url {
                extra.insert("url".to_string(), serde_json::Value::String(u));
            }

            out.push(Resource {
                resource_type: "grafana_datasource".to_string(),
                name: Some(name),
                path: Some(display.clone()),
                provider: None,
                created: None,
                created_by: None,
                detected_by: "grafana".to_string(),
                estimated_cost: None,
                extra,
            });
        }
    }

    if let Some(entries) = doc.get("providers").and_then(|v| v.as_sequence()) {
        for prov in entries {
            let name = prov
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed")
                .to_string();
            let folder = prov
                .get("folder")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut extra = HashMap::new();
            if let Some(f) = folder {
                extra.insert("folder".to_string(), serde_json::Value::String(f));
            }

            out.push(Resource {
                resource_type: "grafana_dashboard_provider".to_string(),
                name: Some(name),
                path: Some(display.clone()),
                provider: None,
                created: None,
                created_by: None,
                detected_by: "grafana".to_string(),
                estimated_cost: None,
                extra,
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parses_dashboard_with_schema_version_and_panels() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("dashboard.json");
        fs::write(
            &path,
            r#"{
              "title": "API Latency",
              "uid": "api-lat-01",
              "schemaVersion": 39,
              "panels": [{"id": 1, "type": "timeseries"}, {"id": 2, "type": "stat"}]
            }"#,
        )
        .unwrap();

        let r = parse_dashboard(&path, tmp.path()).expect("dashboard parsed");
        assert_eq!(r.resource_type, "grafana_dashboard");
        assert_eq!(r.name.as_deref(), Some("API Latency"));
        assert_eq!(
            r.extra.get("uid").and_then(|v| v.as_str()),
            Some("api-lat-01")
        );
        assert_eq!(r.extra.get("panels").and_then(|v| v.as_u64()), Some(2));
    }

    #[test]
    fn ignores_non_grafana_json() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("config.json");
        fs::write(&path, r#"{"name": "widgetshop", "version": "1.0"}"#).unwrap();
        assert!(parse_dashboard(&path, tmp.path()).is_none());
    }

    #[test]
    fn parses_datasource_provisioning() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("prometheus.yaml");
        fs::write(
            &path,
            r#"
apiVersion: 1
datasources:
  - name: Prometheus
    type: prometheus
    url: http://prometheus:9090
  - name: Loki
    type: loki
    url: http://loki:3100
"#,
        )
        .unwrap();

        let resources = parse_provisioning(&path, tmp.path());
        assert_eq!(resources.len(), 2);
        let names: Vec<&str> = resources.iter().filter_map(|r| r.name.as_deref()).collect();
        assert!(names.contains(&"Prometheus"));
        assert!(names.contains(&"Loki"));
        assert_eq!(resources[0].resource_type, "grafana_datasource");
    }

    #[test]
    fn parses_dashboard_provider_provisioning() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("dashboards.yaml");
        fs::write(
            &path,
            r#"
apiVersion: 1
providers:
  - name: default
    folder: Services
    type: file
    options:
      path: /var/lib/grafana/dashboards
"#,
        )
        .unwrap();

        let resources = parse_provisioning(&path, tmp.path());
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].resource_type, "grafana_dashboard_provider");
        assert_eq!(resources[0].name.as_deref(), Some("default"));
    }
}
