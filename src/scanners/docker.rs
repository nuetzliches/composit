use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct DockerScanner;

#[async_trait]
impl Scanner for DockerScanner {
    fn id(&self) -> &str {
        "docker"
    }

    fn name(&self) -> &str {
        "Docker Scanner"
    }

    fn description(&self) -> &str {
        "Detects docker-compose.yml and Dockerfile files"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        // Scan for docker-compose files
        for pattern in &["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"] {
            let full_pattern = context.dir.join("**").join(pattern);
            let pattern_str = full_pattern.to_string_lossy().to_string();
            for entry in glob(&pattern_str)? {
                if let Ok(path) = entry {
                    let resource = scan_compose_file(&path, &context.dir)?;
                    resources.push(resource);
                }
            }
        }

        // Scan for Dockerfiles
        let dockerfile_pattern = context.dir.join("**/Dockerfile*");
        for entry in glob(&dockerfile_pattern.to_string_lossy())? {
            if let Ok(path) = entry {
                let rel_path = path
                    .strip_prefix(&context.dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                resources.push(Resource {
                    resource_type: "dockerfile".to_string(),
                    name: None,
                    path: Some(format!("./{}", rel_path)),
                    provider: None,
                    created: None,
                    created_by: None,
                    detected_by: "docker".to_string(),
                    estimated_cost: None,
                    extra: HashMap::new(),
                });
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}

fn scan_compose_file(path: &Path, base_dir: &Path) -> Result<Resource> {
    let content = std::fs::read_to_string(path)?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

    let services_count = yaml
        .get("services")
        .and_then(|s| s.as_mapping())
        .map(|m| m.len())
        .unwrap_or(0);

    let rel_path = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let mut extra = HashMap::new();
    extra.insert(
        "services".to_string(),
        serde_json::Value::Number(serde_json::Number::from(services_count)),
    );

    Ok(Resource {
        resource_type: "docker_compose".to_string(),
        name: None,
        path: Some(format!("./{}", rel_path)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "docker".to_string(),
        estimated_cost: None,
        extra,
    })
}
