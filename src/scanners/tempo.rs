use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;
use serde_yaml::Value;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct TempoScanner;

#[async_trait]
impl Scanner for TempoScanner {
    fn id(&self) -> &str {
        "tempo"
    }

    fn name(&self) -> &str {
        "Tempo Scanner"
    }

    fn description(&self) -> &str {
        "Detects Grafana Tempo config — receivers, storage backend"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        for pattern in &["**/tempo.yml", "**/tempo.yaml", "**/tempo/**/*.yml", "**/tempo/**/*.yaml"] {
            let full_pattern = context.dir.join(pattern);
            for entry in glob(&full_pattern.to_string_lossy())?.flatten() {
                if context.is_excluded(&entry) || !entry.is_file() {
                    continue;
                }
                if let Some(r) = parse_tempo(&entry, &context.dir) {
                    resources.push(r);
                }
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}

fn parse_tempo(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;
    let doc: Value = serde_yaml::from_str(&content).ok()?;

    // Guard: must look like a Tempo config
    if !looks_like_tempo(&doc) {
        return None;
    }

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let receivers = collect_receivers(&doc);
    let storage_backend = detect_storage(&doc);

    let mut extra = HashMap::new();
    if !receivers.is_empty() {
        extra.insert(
            "receivers".to_string(),
            serde_json::Value::Array(
                receivers.into_iter().map(serde_json::Value::String).collect(),
            ),
        );
    }
    if let Some(backend) = storage_backend {
        extra.insert(
            "storage_backend".to_string(),
            serde_json::Value::String(backend),
        );
    }

    Some(Resource {
        resource_type: "tempo_config".to_string(),
        name: None,
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "tempo".to_string(),
        estimated_cost: None,
        extra,
    })
}

fn looks_like_tempo(doc: &Value) -> bool {
    // Tempo configs always have a `distributor` or `storage` top-level key
    // together with `server` or `ingester`. Two known keys required.
    let known = ["distributor", "storage", "ingester", "compactor", "querier", "server"];
    known.iter().filter(|k| doc.get(k).is_some()).count() >= 2
}

fn collect_receivers(doc: &Value) -> Vec<String> {
    let known = ["otlp", "jaeger", "zipkin", "opencensus", "kafka"];
    let receivers_map = doc
        .get("distributor")
        .and_then(|v| v.get("receivers"))
        .or_else(|| doc.get("receivers"));

    if let Some(map) = receivers_map.and_then(|v: &Value| v.as_mapping()) {
        return map
            .keys()
            .filter_map(|k: &Value| k.as_str().map(str::to_string))
            .filter(|k| known.contains(&k.as_str()))
            .collect();
    }
    // Some configs list receivers as a sequence of names
    if let Some(seq) = receivers_map.and_then(|v: &Value| v.as_sequence()) {
        return seq
            .iter()
            .filter_map(|v: &Value| v.as_str().map(str::to_string))
            .collect();
    }
    vec![]
}

fn detect_storage(doc: &Value) -> Option<String> {
    let backends = ["s3", "gcs", "azure", "swift", "filesystem", "local"];
    let traces = doc.get("storage").and_then(|v| v.get("trace"))?;
    if let Some(mapping) = traces.as_mapping() {
        for backend in &backends {
            if mapping.contains_key(&Value::String(backend.to_string())) {
                return Some(backend.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(yaml: &str) -> Value {
        serde_yaml::from_str(yaml).unwrap()
    }

    #[test]
    fn fingerprint_requires_two_known_keys() {
        assert!(!looks_like_tempo(&doc("server:\n  http_listen_port: 3200\n")));
        assert!(looks_like_tempo(&doc(
            "server:\n  http_listen_port: 3200\ndistributor:\n  receivers: {}\n"
        )));
    }

    #[test]
    fn detects_s3_storage_backend() {
        let yaml = r#"
server:
  http_listen_port: 3200
storage:
  trace:
    s3:
      bucket: tempo-traces
      endpoint: s3.amazonaws.com
"#;
        assert_eq!(detect_storage(&doc(yaml)), Some("s3".to_string()));
    }

    #[test]
    fn collects_receivers_from_distributor() {
        let yaml = r#"
distributor:
  receivers:
    otlp:
      protocols:
        grpc:
    jaeger:
      protocols:
        thrift_http:
"#;
        let receivers = collect_receivers(&doc(yaml));
        assert!(receivers.contains(&"otlp".to_string()));
        assert!(receivers.contains(&"jaeger".to_string()));
    }
}
