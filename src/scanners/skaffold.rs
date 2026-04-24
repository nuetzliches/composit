use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;
use serde_yaml::Value;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct SkaffoldScanner;

#[async_trait]
impl Scanner for SkaffoldScanner {
    fn id(&self) -> &str {
        "skaffold"
    }

    fn name(&self) -> &str {
        "Skaffold Scanner"
    }

    fn description(&self) -> &str {
        "Detects skaffold.yaml — build artifacts, deploy type, profiles"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        for pattern in &["**/skaffold.yaml", "**/skaffold.yml"] {
            let full_pattern = context.dir.join(pattern);
            for entry in glob(&full_pattern.to_string_lossy())?.flatten() {
                if context.is_excluded(&entry) || !entry.is_file() {
                    continue;
                }
                if let Some(r) = parse_skaffold(&entry, &context.dir) {
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

fn parse_skaffold(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;
    let doc: Value = serde_yaml::from_str(&content).ok()?;

    // Guard: must be a Skaffold config (apiVersion starts with "skaffold/")
    let api_version = doc.get("apiVersion").and_then(|v| v.as_str()).unwrap_or("");
    if !api_version.starts_with("skaffold/") {
        return None;
    }

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let name = doc
        .get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let artifacts = doc
        .get("build")
        .and_then(|v| v.get("artifacts"))
        .and_then(|v: &Value| v.as_sequence())
        .map(|s| s.len())
        .unwrap_or(0);

    let deploy_type = detect_deploy_type(&doc);
    let profiles = doc
        .get("profiles")
        .and_then(|v| v.as_sequence())
        .map(|s| s.len())
        .unwrap_or(0);

    let mut extra = HashMap::new();
    extra.insert(
        "artifacts".to_string(),
        serde_json::Value::Number(serde_json::Number::from(artifacts)),
    );
    extra.insert(
        "profiles".to_string(),
        serde_json::Value::Number(serde_json::Number::from(profiles)),
    );
    if let Some(dt) = deploy_type {
        extra.insert("deploy_type".to_string(), serde_json::Value::String(dt));
    }

    Some(Resource {
        resource_type: "skaffold_config".to_string(),
        name,
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "skaffold".to_string(),
        estimated_cost: None,
        extra,
    })
}

fn detect_deploy_type(doc: &Value) -> Option<String> {
    let deploy = doc.get("deploy")?;
    for key in &["helm", "kubectl", "kustomize"] {
        if deploy.get(key).is_some() {
            return Some(key.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(yaml: &str) -> Value {
        serde_yaml::from_str(yaml).unwrap()
    }

    #[test]
    fn rejects_non_skaffold_yaml() {
        let doc = make_doc("apiVersion: apps/v1\nkind: Deployment\n");
        assert!(detect_deploy_type(&doc).is_none());
        // apiVersion guard
        let api = doc.get("apiVersion").and_then(|v| v.as_str()).unwrap_or("");
        assert!(!api.starts_with("skaffold/"));
    }

    #[test]
    fn detects_helm_deploy() {
        let doc = make_doc("apiVersion: skaffold/v4beta11\ndeploy:\n  helm:\n    releases: []\n");
        assert_eq!(detect_deploy_type(&doc), Some("helm".to_string()));
    }

    #[test]
    fn counts_artifacts_and_profiles() {
        let yaml = r#"
apiVersion: skaffold/v4beta11
build:
  artifacts:
    - image: gcr.io/my-project/api
    - image: gcr.io/my-project/worker
deploy:
  kubectl:
    manifests: ["k8s/*.yaml"]
profiles:
  - name: staging
  - name: production
"#;
        let doc = make_doc(yaml);
        let artifacts = doc
            .get("build")
            .and_then(|v| v.get("artifacts"))
            .and_then(|v: &Value| v.as_sequence())
            .map(|s| s.len())
            .unwrap_or(0);
        let profiles = doc
            .get("profiles")
            .and_then(|v: &Value| v.as_sequence())
            .map(|s| s.len())
            .unwrap_or(0);
        assert_eq!(artifacts, 2);
        assert_eq!(profiles, 2);
        assert_eq!(detect_deploy_type(&doc), Some("kubectl".to_string()));
    }
}
