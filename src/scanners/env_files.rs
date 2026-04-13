use std::collections::HashMap;
use std::fs;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct EnvFilesScanner;

#[async_trait]
impl Scanner for EnvFilesScanner {
    fn id(&self) -> &str {
        "env_files"
    }

    fn name(&self) -> &str {
        "Environment Files Scanner"
    }

    fn description(&self) -> &str {
        "Detects .env files and counts variables (does not read values)"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        for pattern in &[".env", ".env.*"] {
            let full_pattern = context.dir.join("**").join(pattern);
            for entry in glob(&full_pattern.to_string_lossy())? {
                if let Ok(path) = entry {
                    // Skip .env files inside node_modules, target, .git etc.
                    let path_str = path.to_string_lossy();
                    if path_str.contains("node_modules")
                        || path_str.contains("/target/")
                        || path_str.contains("/.git/")
                    {
                        continue;
                    }

                    let content = fs::read_to_string(&path).unwrap_or_default();
                    let var_count = content
                        .lines()
                        .filter(|line| {
                            let trimmed = line.trim();
                            !trimmed.is_empty()
                                && !trimmed.starts_with('#')
                                && trimmed.contains('=')
                        })
                        .count();

                    let rel_path = path
                        .strip_prefix(&context.dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();

                    let mut extra = HashMap::new();
                    extra.insert(
                        "variables".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(var_count)),
                    );

                    resources.push(Resource {
                        resource_type: "env_file".to_string(),
                        name: None,
                        path: Some(format!("./{}", rel_path)),
                        provider: None,
                        created: None,
                        created_by: None,
                        detected_by: "env_files".to_string(),
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
