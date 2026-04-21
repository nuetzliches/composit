use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;
use serde::Deserialize;
use serde_yaml::Value;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct KubernetesScanner;

#[async_trait]
impl Scanner for KubernetesScanner {
    fn id(&self) -> &str {
        "kubernetes"
    }

    fn name(&self) -> &str {
        "Kubernetes Scanner"
    }

    fn description(&self) -> &str {
        "Detects Kubernetes manifests, Kustomize overlays and Helm charts"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        for pattern in &["**/*.yaml", "**/*.yml"] {
            let full_pattern = context.dir.join(pattern);
            for entry in glob(&full_pattern.to_string_lossy())?.flatten() {
                if should_skip(&entry) {
                    continue;
                }

                let rel = entry
                    .strip_prefix(&context.dir)
                    .unwrap_or(&entry)
                    .to_string_lossy()
                    .to_string();
                let display_path = format!("./{}", rel);

                let content = match std::fs::read_to_string(&entry) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let file_name = entry.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if file_name == "Chart.yaml" {
                    if let Some(r) = parse_helm_chart(&content, &display_path) {
                        resources.push(r);
                    }
                    continue;
                }

                if is_kustomization(file_name) {
                    if let Some(r) = parse_kustomization(&content, &display_path) {
                        resources.push(r);
                    }
                    continue;
                }

                resources.extend(parse_k8s_manifests(&content, &display_path));
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}

fn is_kustomization(file_name: &str) -> bool {
    matches!(
        file_name,
        "kustomization.yaml" | "kustomization.yml" | "Kustomization"
    )
}

/// Paths owned by other scanners (docker, prometheus, workflows, gitlab-ci)
/// must not also surface here, and common vendor/build dirs are noise.
/// Helm chart `templates/` are skipped because Go-templated YAML rarely
/// parses and would inflate counts per chart.
fn should_skip(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.contains("/node_modules/")
        || s.contains("/target/")
        || s.contains("/.git/")
        || s.contains("/vendor/")
        || s.contains("/templates/")
    {
        return true;
    }

    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if (name.starts_with("docker-compose") || name == "compose.yml" || name == "compose.yaml")
        && (name.ends_with(".yml") || name.ends_with(".yaml"))
    {
        return true;
    }
    if matches!(
        name,
        "prometheus.yml"
            | "prometheus.yaml"
            | "alerts.yml"
            | "alerts.yaml"
            | "alert_rules.yml"
            | "alert_rules.yaml"
            | ".gitlab-ci.yml"
            | ".gitlab-ci.yaml"
    ) {
        return true;
    }
    if s.contains("/.github/workflows/")
        || s.contains("/.gitea/workflows/")
        || s.contains("/.forgejo/workflows/")
    {
        return true;
    }
    false
}

fn parse_helm_chart(content: &str, display_path: &str) -> Option<Resource> {
    let doc: Value = serde_yaml::from_str(content).ok()?;
    let name = doc.get("name").and_then(|v| v.as_str())?;
    let version = doc
        .get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let api_version = doc
        .get("apiVersion")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let dependencies = doc
        .get("dependencies")
        .and_then(|v| v.as_sequence())
        .map(|s| s.len())
        .unwrap_or(0);

    let mut extra = HashMap::new();
    if let Some(v) = version {
        extra.insert("chart_version".to_string(), serde_json::Value::String(v));
    }
    if let Some(v) = api_version {
        extra.insert("api_version".to_string(), serde_json::Value::String(v));
    }
    extra.insert(
        "dependencies".to_string(),
        serde_json::Value::Number(serde_json::Number::from(dependencies)),
    );

    Some(Resource {
        resource_type: "helm_chart".to_string(),
        name: Some(name.to_string()),
        path: Some(display_path.to_string()),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "kubernetes".to_string(),
        estimated_cost: None,
        extra,
    })
}

fn parse_kustomization(content: &str, display_path: &str) -> Option<Resource> {
    let doc: Value = serde_yaml::from_str(content).ok()?;
    let resources_count = doc
        .get("resources")
        .and_then(|v| v.as_sequence())
        .map(|s| s.len())
        .unwrap_or(0);
    let bases_count = doc
        .get("bases")
        .and_then(|v| v.as_sequence())
        .map(|s| s.len())
        .unwrap_or(0);
    let components_count = doc
        .get("components")
        .and_then(|v| v.as_sequence())
        .map(|s| s.len())
        .unwrap_or(0);
    let namespace = doc
        .get("namespace")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut extra = HashMap::new();
    extra.insert(
        "resources".to_string(),
        serde_json::Value::Number(serde_json::Number::from(resources_count)),
    );
    if bases_count > 0 {
        extra.insert(
            "bases".to_string(),
            serde_json::Value::Number(serde_json::Number::from(bases_count)),
        );
    }
    if components_count > 0 {
        extra.insert(
            "components".to_string(),
            serde_json::Value::Number(serde_json::Number::from(components_count)),
        );
    }
    if let Some(ns) = namespace {
        extra.insert("namespace".to_string(), serde_json::Value::String(ns));
    }

    Some(Resource {
        resource_type: "kustomization".to_string(),
        name: None,
        path: Some(display_path.to_string()),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "kubernetes".to_string(),
        estimated_cost: None,
        extra,
    })
}

/// A single YAML file can carry many K8s manifests separated by `---`.
/// Each document that has both `apiVersion` and `kind` becomes its own
/// resource so governance rules can target them individually.
fn parse_k8s_manifests(content: &str, display_path: &str) -> Vec<Resource> {
    let mut out = Vec::new();
    for doc in serde_yaml::Deserializer::from_str(content) {
        let value = match Value::deserialize(doc) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(r) = manifest_from_doc(&value, display_path) {
            out.push(r);
        }
    }
    out
}

fn manifest_from_doc(doc: &Value, display_path: &str) -> Option<Resource> {
    let api_version = doc.get("apiVersion")?.as_str()?;
    let kind = doc.get("kind")?.as_str()?;
    let metadata = doc.get("metadata");
    let metadata_name = metadata
        .and_then(|m| m.get("name"))
        .and_then(|v| v.as_str());
    let namespace = metadata
        .and_then(|m| m.get("namespace"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Qualify the resource name with its kind so a Deployment and a Service
    // named "api" in the same file don't collapse under dedup_resources
    // (which keys on type+name+path). Matches the `kubectl get deployment/api`
    // shorthand most operators already recognise.
    let name = metadata_name.map(|n| format!("{}/{}", kind, n));

    let mut extra = HashMap::new();
    extra.insert(
        "kind".to_string(),
        serde_json::Value::String(kind.to_string()),
    );
    extra.insert(
        "api_version".to_string(),
        serde_json::Value::String(api_version.to_string()),
    );
    if let Some(ns) = namespace {
        extra.insert("namespace".to_string(), serde_json::Value::String(ns));
    }

    Some(Resource {
        resource_type: "kubernetes_manifest".to_string(),
        name,
        path: Some(display_path.to_string()),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "kubernetes".to_string(),
        estimated_cost: None,
        extra,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_deployment() {
        let yaml = r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: api
  namespace: widgetshop
spec:
  replicas: 3
"#;
        let resources = parse_k8s_manifests(yaml, "./deployment.yaml");
        assert_eq!(resources.len(), 1);
        let r = &resources[0];
        assert_eq!(r.resource_type, "kubernetes_manifest");
        assert_eq!(r.name.as_deref(), Some("Deployment/api"));
        assert_eq!(
            r.extra.get("kind").and_then(|v| v.as_str()),
            Some("Deployment")
        );
        assert_eq!(
            r.extra.get("namespace").and_then(|v| v.as_str()),
            Some("widgetshop")
        );
    }

    #[test]
    fn test_parse_multi_document_yaml() {
        // A single file often contains Deployment + Service — both must surface.
        let yaml = r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: api
---
apiVersion: v1
kind: Service
metadata:
  name: api-svc
"#;
        let resources = parse_k8s_manifests(yaml, "./all.yaml");
        assert_eq!(resources.len(), 2);
        let kinds: Vec<&str> = resources
            .iter()
            .filter_map(|r| r.extra.get("kind").and_then(|v| v.as_str()))
            .collect();
        assert!(kinds.contains(&"Deployment"));
        assert!(kinds.contains(&"Service"));
    }

    #[test]
    fn test_plain_yaml_without_kind_is_ignored() {
        let yaml = "name: just-some-config\nvalue: 42\n";
        assert!(parse_k8s_manifests(yaml, "./config.yaml").is_empty());
    }

    #[test]
    fn test_parse_kustomization() {
        let yaml = r#"
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
namespace: widgetshop
resources:
  - deployment.yaml
  - service.yaml
bases:
  - ../base
"#;
        let r = parse_kustomization(yaml, "./kustomization.yaml").unwrap();
        assert_eq!(r.resource_type, "kustomization");
        assert_eq!(r.extra.get("resources").and_then(|v| v.as_u64()), Some(2));
        assert_eq!(r.extra.get("bases").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(
            r.extra.get("namespace").and_then(|v| v.as_str()),
            Some("widgetshop")
        );
    }

    #[test]
    fn test_parse_helm_chart() {
        let yaml = r#"
apiVersion: v2
name: widgetshop
version: 0.3.1
description: Widgetshop Helm chart
dependencies:
  - name: postgres
    version: 15.0.0
"#;
        let r = parse_helm_chart(yaml, "./chart/Chart.yaml").unwrap();
        assert_eq!(r.resource_type, "helm_chart");
        assert_eq!(r.name.as_deref(), Some("widgetshop"));
        assert_eq!(
            r.extra.get("chart_version").and_then(|v| v.as_str()),
            Some("0.3.1")
        );
        assert_eq!(
            r.extra.get("dependencies").and_then(|v| v.as_u64()),
            Some(1)
        );
    }

    #[test]
    fn test_should_skip_vendor_and_other_scanner_files() {
        assert!(should_skip(Path::new("/repo/node_modules/foo/bar.yaml")));
        assert!(should_skip(Path::new(
            "/repo/chart/templates/deployment.yaml"
        )));
        assert!(should_skip(Path::new("/repo/docker-compose.yml")));
        assert!(should_skip(Path::new("/repo/prometheus.yml")));
        assert!(should_skip(Path::new("/repo/.github/workflows/ci.yml")));
        assert!(!should_skip(Path::new("/repo/k8s/deployment.yaml")));
    }
}
