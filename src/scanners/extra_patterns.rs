use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::config::ExtraPattern;
use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

/// Dynamic scanner created from extra_patterns in composit.config.yaml
pub struct ExtraPatternsScanner {
    pub patterns: Vec<ExtraPattern>,
}

#[async_trait]
impl Scanner for ExtraPatternsScanner {
    fn id(&self) -> &str {
        "extra_patterns"
    }

    fn name(&self) -> &str {
        "Extra Patterns Scanner"
    }

    fn description(&self) -> &str {
        "Scans for custom file patterns defined in composit.config.yaml"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        for pattern in &self.patterns {
            let full_pattern = context.dir.join(&pattern.glob);
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

                    let mut extra = HashMap::new();
                    if let Some(desc) = &pattern.description {
                        extra.insert(
                            "description".to_string(),
                            serde_json::Value::String(desc.clone()),
                        );
                    }

                    resources.push(Resource {
                        resource_type: pattern.resource_type.clone(),
                        name: None,
                        path: Some(format!("./{}", rel_path)),
                        provider: None,
                        created: None,
                        created_by: None,
                        detected_by: "extra_patterns".to_string(),
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
