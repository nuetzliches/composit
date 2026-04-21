use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct PrometheusScanner;

#[async_trait]
impl Scanner for PrometheusScanner {
    fn id(&self) -> &str {
        "prometheus"
    }

    fn name(&self) -> &str {
        "Prometheus Scanner"
    }

    fn description(&self) -> &str {
        "Detects Prometheus configuration and alerting rules"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        // Scan for prometheus.yml / prometheus.yaml
        for pattern in &["**/prometheus.yml", "**/prometheus.yaml"] {
            let full_pattern = context.dir.join(pattern);
            for entry in glob(&full_pattern.to_string_lossy())? {
                if let Ok(path) = entry {
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
                                "warning: prometheus scanner could not read {}: {}",
                                path.display(),
                                e
                            );
                            continue;
                        }
                    };

                    let yaml: serde_yaml::Value = match serde_yaml::from_str(&content) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!(
                                "warning: prometheus scanner could not parse {}: {}",
                                path.display(),
                                e
                            );
                            continue;
                        }
                    };

                    let mut extra = HashMap::new();

                    // Extract scrape configs
                    let scrape_configs = yaml
                        .get("scrape_configs")
                        .and_then(|s| s.as_sequence())
                        .map(|s| s.len())
                        .unwrap_or(0);
                    extra.insert(
                        "scrape_configs".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(scrape_configs)),
                    );

                    // Extract job names
                    let job_names: Vec<String> = yaml
                        .get("scrape_configs")
                        .and_then(|s| s.as_sequence())
                        .map(|configs| {
                            configs
                                .iter()
                                .filter_map(|c| {
                                    c.get("job_name")
                                        .and_then(|j| j.as_str())
                                        .map(|s| s.to_string())
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    if !job_names.is_empty() {
                        extra.insert(
                            "job_names".to_string(),
                            serde_json::Value::Array(
                                job_names
                                    .into_iter()
                                    .map(serde_json::Value::String)
                                    .collect(),
                            ),
                        );
                    }

                    // Extract rule files
                    let rule_files: Vec<String> = yaml
                        .get("rule_files")
                        .and_then(|r| r.as_sequence())
                        .map(|files| {
                            files
                                .iter()
                                .filter_map(|f| f.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    if !rule_files.is_empty() {
                        extra.insert(
                            "rule_files".to_string(),
                            serde_json::Value::Array(
                                rule_files
                                    .into_iter()
                                    .map(serde_json::Value::String)
                                    .collect(),
                            ),
                        );
                    }

                    // Detect alertmanager config
                    if yaml.get("alerting").is_some() {
                        extra.insert("alerting".to_string(), serde_json::Value::Bool(true));
                    }

                    // Detect remote write/read
                    if yaml.get("remote_write").is_some() {
                        extra.insert("remote_write".to_string(), serde_json::Value::Bool(true));
                    }
                    if yaml.get("remote_read").is_some() {
                        extra.insert("remote_read".to_string(), serde_json::Value::Bool(true));
                    }

                    resources.push(Resource {
                        resource_type: "prometheus_config".to_string(),
                        name: None,
                        path: Some(display_path),
                        provider: None,
                        created: None,
                        created_by: None,
                        detected_by: "prometheus".to_string(),
                        estimated_cost: None,
                        extra,
                    });
                }
            }
        }

        // Scan for alerting rule files
        for pattern in &[
            "**/alerts.yml",
            "**/alerts.yaml",
            "**/alert_rules.yml",
            "**/alert_rules.yaml",
            "**/rules/*.yml",
            "**/rules/*.yaml",
        ] {
            let full_pattern = context.dir.join(pattern);
            for entry in glob(&full_pattern.to_string_lossy())? {
                if let Ok(path) = entry {
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
                                "warning: prometheus rules scanner could not read {}: {}",
                                path.display(),
                                e
                            );
                            continue;
                        }
                    };

                    let yaml: serde_yaml::Value = match serde_yaml::from_str(&content) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!(
                                "warning: prometheus rules scanner could not parse {}: {}",
                                path.display(),
                                e
                            );
                            continue;
                        }
                    };

                    // Only process if it looks like a Prometheus rules file
                    if yaml.get("groups").is_none() {
                        continue;
                    }

                    let groups = yaml
                        .get("groups")
                        .and_then(|g| g.as_sequence())
                        .map(|g| g.len())
                        .unwrap_or(0);

                    let rule_count: usize = yaml
                        .get("groups")
                        .and_then(|g| g.as_sequence())
                        .map(|groups| {
                            groups
                                .iter()
                                .filter_map(|g| g.get("rules").and_then(|r| r.as_sequence()))
                                .map(|r| r.len())
                                .sum()
                        })
                        .unwrap_or(0);

                    let mut extra = HashMap::new();
                    extra.insert(
                        "groups".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(groups)),
                    );
                    extra.insert(
                        "rules".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(rule_count)),
                    );

                    resources.push(Resource {
                        resource_type: "prometheus_rules".to_string(),
                        name: None,
                        path: Some(display_path),
                        provider: None,
                        created: None,
                        created_by: None,
                        detected_by: "prometheus".to_string(),
                        estimated_cost: None,
                        extra,
                    });
                }
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_prometheus_config() {
        let yaml: serde_yaml::Value = serde_yaml::from_str(
            r#"
global:
  scrape_interval: 15s

rule_files:
  - "alerts.yml"

alerting:
  alertmanagers:
    - static_configs:
        - targets: ["alertmanager:9093"]

scrape_configs:
  - job_name: "prometheus"
    static_configs:
      - targets: ["localhost:9090"]

  - job_name: "node"
    static_configs:
      - targets: ["node-exporter:9100"]

  - job_name: "cadvisor"
    static_configs:
      - targets: ["cadvisor:8080"]
"#,
        )
        .unwrap();

        let scrape_configs = yaml
            .get("scrape_configs")
            .and_then(|s| s.as_sequence())
            .map(|s| s.len())
            .unwrap_or(0);
        assert_eq!(scrape_configs, 3);

        let job_names: Vec<String> = yaml
            .get("scrape_configs")
            .and_then(|s| s.as_sequence())
            .unwrap()
            .iter()
            .filter_map(|c| {
                c.get("job_name")
                    .and_then(|j| j.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        assert_eq!(job_names, vec!["prometheus", "node", "cadvisor"]);

        assert!(yaml.get("alerting").is_some());
    }

    #[test]
    fn test_parse_alert_rules() {
        let yaml: serde_yaml::Value = serde_yaml::from_str(
            r#"
groups:
  - name: node_alerts
    rules:
      - alert: HighCPU
        expr: node_cpu_seconds_total > 0.9
        for: 5m
      - alert: LowDisk
        expr: node_filesystem_avail_bytes < 1e9
        for: 10m

  - name: service_alerts
    rules:
      - alert: ServiceDown
        expr: up == 0
        for: 1m
"#,
        )
        .unwrap();

        let groups = yaml.get("groups").and_then(|g| g.as_sequence()).unwrap();
        assert_eq!(groups.len(), 2);

        let rule_count: usize = groups
            .iter()
            .filter_map(|g| g.get("rules").and_then(|r| r.as_sequence()))
            .map(|r| r.len())
            .sum();
        assert_eq!(rule_count, 3);
    }
}
