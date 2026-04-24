use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct VercelJsonScanner;

#[async_trait]
impl Scanner for VercelJsonScanner {
    fn id(&self) -> &str {
        "vercel_json"
    }

    fn name(&self) -> &str {
        "Vercel Scanner"
    }

    fn description(&self) -> &str {
        "Detects vercel.json — rewrites, redirects, headers, functions"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        let pattern = context.dir.join("**/vercel.json");
        for entry in glob(&pattern.to_string_lossy())?.flatten() {
            if context.is_excluded(&entry) || !entry.is_file() {
                continue;
            }
            if let Some(r) = parse_vercel_json(&entry, &context.dir) {
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

fn parse_vercel_json(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;
    let doc: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Must be a JSON object to be a valid vercel.json
    doc.as_object()?;

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let rewrites = array_len(&doc, "rewrites");
    let redirects = array_len(&doc, "redirects");
    let headers = array_len(&doc, "headers");
    let functions = doc
        .get("functions")
        .and_then(|v| v.as_object())
        .map(|o| o.len())
        .unwrap_or(0);

    let mut extra = HashMap::new();
    extra.insert(
        "rewrites".to_string(),
        serde_json::Value::Number(serde_json::Number::from(rewrites)),
    );
    extra.insert(
        "redirects".to_string(),
        serde_json::Value::Number(serde_json::Number::from(redirects)),
    );
    extra.insert(
        "headers".to_string(),
        serde_json::Value::Number(serde_json::Number::from(headers)),
    );
    extra.insert(
        "functions".to_string(),
        serde_json::Value::Number(serde_json::Number::from(functions)),
    );

    if let Some(framework) = doc.get("framework").and_then(|v| v.as_str()) {
        extra.insert(
            "framework".to_string(),
            serde_json::Value::String(framework.to_string()),
        );
    }

    Some(Resource {
        resource_type: "vercel_config".to_string(),
        name: None,
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "vercel_json".to_string(),
        estimated_cost: None,
        extra,
    })
}

fn array_len(doc: &serde_json::Value, key: &str) -> usize {
    doc.get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_len_counts_entries() {
        let doc = serde_json::json!({
            "rewrites": [{"source": "/old", "destination": "/new"}],
            "redirects": []
        });
        assert_eq!(array_len(&doc, "rewrites"), 1);
        assert_eq!(array_len(&doc, "redirects"), 0);
        assert_eq!(array_len(&doc, "headers"), 0);
    }

    #[test]
    fn functions_counts_object_keys() {
        let doc = serde_json::json!({
            "functions": {
                "api/users.js": {"memory": 128},
                "api/auth.js": {"memory": 256}
            }
        });
        let functions = doc
            .get("functions")
            .and_then(|v| v.as_object())
            .map(|o| o.len())
            .unwrap_or(0);
        assert_eq!(functions, 2);
    }

    #[test]
    fn non_object_json_is_rejected() {
        let doc: serde_json::Value = serde_json::from_str("[]").unwrap();
        assert!(doc.as_object().is_none());
    }
}
