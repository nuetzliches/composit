use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;
use serde_yaml::Value;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct TraefikScanner;

#[async_trait]
impl Scanner for TraefikScanner {
    fn id(&self) -> &str {
        "traefik"
    }

    fn name(&self) -> &str {
        "Traefik Scanner"
    }

    fn description(&self) -> &str {
        "Detects traefik.yml/toml — entrypoints, providers, dashboard, TLS"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        // YAML variants
        for pattern in &[
            "**/traefik.yml",
            "**/traefik.yaml",
            "**/traefik/**/*.yml",
            "**/traefik/**/*.yaml",
        ] {
            let full_pattern = context.dir.join(pattern);
            for entry in glob(&full_pattern.to_string_lossy())?.flatten() {
                if context.is_excluded(&entry) || !entry.is_file() {
                    continue;
                }
                if let Some(r) = parse_traefik_yaml(&entry, &context.dir) {
                    resources.push(r);
                }
            }
        }

        // TOML variants
        for pattern in &["**/traefik.toml", "**/traefik/**/*.toml"] {
            let full_pattern = context.dir.join(pattern);
            for entry in glob(&full_pattern.to_string_lossy())?.flatten() {
                if context.is_excluded(&entry) || !entry.is_file() {
                    continue;
                }
                if let Some(r) = parse_traefik_toml(&entry, &context.dir) {
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

fn parse_traefik_yaml(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;
    let doc: Value = serde_yaml::from_str(&content).ok()?;

    // Must have at least one known Traefik top-level key
    if !looks_like_traefik_yaml(&doc) {
        return None;
    }

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let entrypoints = collect_yaml_map_keys(&doc, "entryPoints");
    let providers = collect_yaml_map_keys(&doc, "providers");
    let dashboard = doc
        .get("api")
        .and_then(|v| v.get("dashboard"))
        .and_then(|v: &Value| v.as_bool())
        .unwrap_or(false)
        || doc.get("api").is_some();
    let tls = doc.get("certificatesResolvers").is_some();

    build_resource(rel, entrypoints, providers, dashboard, tls, "traefik")
}

fn parse_traefik_toml(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;

    if !looks_like_traefik_toml(&content) {
        return None;
    }

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let entrypoints = extract_toml_section_keys(&content, "entryPoints");
    let providers = extract_toml_section_keys(&content, "providers");
    let dashboard = content.contains("[api]") || content.contains("dashboard = true");
    let tls = content.contains("[certificatesResolvers]");

    build_resource(rel, entrypoints, providers, dashboard, tls, "traefik")
}

fn build_resource(
    rel: String,
    entrypoints: Vec<String>,
    providers: Vec<String>,
    dashboard: bool,
    tls: bool,
    detected_by: &str,
) -> Option<Resource> {
    let mut extra = HashMap::new();
    if !entrypoints.is_empty() {
        extra.insert(
            "entrypoints".to_string(),
            serde_json::Value::Array(
                entrypoints
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if !providers.is_empty() {
        extra.insert(
            "providers".to_string(),
            serde_json::Value::Array(
                providers
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if dashboard {
        extra.insert("dashboard".to_string(), serde_json::Value::Bool(true));
    }
    if tls {
        extra.insert("tls".to_string(), serde_json::Value::Bool(true));
    }

    Some(Resource {
        resource_type: "traefik_config".to_string(),
        name: None,
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: detected_by.to_string(),
        estimated_cost: None,
        extra,
    })
}

fn looks_like_traefik_yaml(doc: &Value) -> bool {
    let known_keys = [
        "entryPoints",
        "providers",
        "api",
        "certificatesResolvers",
        "log",
        "accessLog",
        "metrics",
        "tracing",
        "ping",
    ];
    known_keys.iter().filter(|k| doc.get(k).is_some()).count() >= 2
}

fn looks_like_traefik_toml(content: &str) -> bool {
    let known_sections = [
        "[entryPoints",
        "[providers",
        "[api]",
        "[certificatesResolvers",
        "[log]",
        "[accessLog]",
        "[metrics",
        "[tracing",
    ];
    known_sections
        .iter()
        .filter(|s| content.contains(*s))
        .count()
        >= 2
}

fn collect_yaml_map_keys(doc: &Value, key: &str) -> Vec<String> {
    doc.get(key)
        .and_then(|v| v.as_mapping())
        .map(|m| {
            m.keys()
                .filter_map(|k| k.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Extracts sub-keys of a TOML section header, e.g. `[entryPoints.web]` → "web".
fn extract_toml_section_keys(content: &str, section: &str) -> Vec<String> {
    let prefix = format!("[{section}.");
    let mut keys = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(&prefix) {
            let key = rest.trim_end_matches(']').trim().to_string();
            if !key.is_empty() && !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_fingerprint_requires_two_known_keys() {
        let doc: Value =
            serde_yaml::from_str("entryPoints:\n  web:\n    address: ':80'\n").unwrap();
        // only one key — should fail
        assert!(!looks_like_traefik_yaml(&doc));

        let doc2: Value = serde_yaml::from_str(
            "entryPoints:\n  web:\n    address: ':80'\napi:\n  dashboard: true\n",
        )
        .unwrap();
        assert!(looks_like_traefik_yaml(&doc2));
    }

    #[test]
    fn toml_section_key_extraction() {
        let toml = "[entryPoints.web]\n  address = \":80\"\n[entryPoints.websecure]\n  address = \":443\"\n";
        let keys = extract_toml_section_keys(toml, "entryPoints");
        assert_eq!(keys, vec!["web", "websecure"]);
    }

    #[test]
    fn toml_fingerprint_requires_two_sections() {
        let toml = "[entryPoints.web]\n  address = \":80\"\n";
        assert!(!looks_like_traefik_toml(toml));

        let toml2 = "[entryPoints.web]\n  address = \":80\"\n[api]\n  dashboard = true\n";
        assert!(looks_like_traefik_toml(toml2));
    }
}
