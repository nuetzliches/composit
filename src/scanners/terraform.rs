use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct TerraformScanner;

#[async_trait]
impl Scanner for TerraformScanner {
    fn id(&self) -> &str {
        "terraform"
    }

    fn name(&self) -> &str {
        "Terraform Scanner"
    }

    fn description(&self) -> &str {
        "Detects Terraform state files and configuration"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        // Scan for .tfstate files
        let state_pattern = context.dir.join("**/*.tfstate");
        for entry in glob(&state_pattern.to_string_lossy())? {
            if let Ok(path) = entry {
                let rel_path = path
                    .strip_prefix(&context.dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                // Try to count managed resources in state
                let managed_count = if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(state) = serde_json::from_str::<serde_json::Value>(&content) {
                        state
                            .get("resources")
                            .and_then(|r| r.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0)
                    } else {
                        0
                    }
                } else {
                    0
                };

                let mut extra = HashMap::new();
                extra.insert(
                    "managed_resources".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(managed_count)),
                );

                resources.push(Resource {
                    resource_type: "terraform_state".to_string(),
                    name: None,
                    path: Some(format!("./{}", rel_path)),
                    provider: None,
                    created: None,
                    created_by: None,
                    detected_by: "terraform".to_string(),
                    estimated_cost: None,
                    extra,
                });
            }
        }

        // Scan for .tf files (just detect presence, group by directory)
        let tf_pattern = context.dir.join("**/*.tf");
        let mut tf_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();
        for entry in glob(&tf_pattern.to_string_lossy())? {
            if let Ok(path) = entry {
                if let Some(parent) = path.parent() {
                    let rel_dir = parent
                        .strip_prefix(&context.dir)
                        .unwrap_or(parent)
                        .to_string_lossy()
                        .to_string();
                    tf_dirs.insert(rel_dir);
                }
            }
        }

        for dir in tf_dirs {
            let display_path = if dir.is_empty() {
                ".".to_string()
            } else {
                format!("./{}", dir)
            };

            resources.push(Resource {
                resource_type: "terraform_config".to_string(),
                name: None,
                path: Some(display_path),
                provider: None,
                created: None,
                created_by: None,
                detected_by: "terraform".to_string(),
                estimated_cost: None,
                extra: HashMap::new(),
            });
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}
