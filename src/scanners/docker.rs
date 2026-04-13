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
        "Detects docker-compose services and Dockerfiles"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        // Scan for docker-compose files
        for pattern in &[
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml",
            "compose.yaml",
        ] {
            let full_pattern = context.dir.join("**").join(pattern);
            let pattern_str = full_pattern.to_string_lossy().to_string();
            for entry in glob(&pattern_str)? {
                if let Ok(path) = entry {
                    let (compose_resource, service_resources) =
                        scan_compose_file(&path, &context.dir)?;
                    resources.push(compose_resource);
                    resources.extend(service_resources);
                }
            }
        }

        // Scan for standalone Dockerfiles (not already covered by compose)
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

fn scan_compose_file(path: &Path, base_dir: &Path) -> Result<(Resource, Vec<Resource>)> {
    let content = std::fs::read_to_string(path)?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

    let rel_path = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let compose_path = format!("./{}", rel_path);

    let services = yaml
        .get("services")
        .and_then(|s| s.as_mapping())
        .cloned()
        .unwrap_or_default();

    let services_count = services.len();

    // Compose file resource
    let mut compose_extra = HashMap::new();
    compose_extra.insert(
        "services".to_string(),
        serde_json::Value::Number(serde_json::Number::from(services_count)),
    );

    let compose_resource = Resource {
        resource_type: "docker_compose".to_string(),
        name: None,
        path: Some(compose_path.clone()),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "docker".to_string(),
        estimated_cost: None,
        extra: compose_extra,
    };

    // Individual service resources
    let mut service_resources = Vec::new();
    for (key, value) in &services {
        let service_name = key.as_str().unwrap_or("unknown").to_string();
        let mut extra = HashMap::new();

        // Image
        if let Some(image) = value.get("image").and_then(|v| v.as_str()) {
            extra.insert(
                "image".to_string(),
                serde_json::Value::String(image.to_string()),
            );
        }

        // Build context
        if let Some(build) = value.get("build") {
            let build_ctx = if let Some(s) = build.as_str() {
                s.to_string()
            } else if let Some(ctx) = build.get("context").and_then(|c| c.as_str()) {
                ctx.to_string()
            } else {
                ".".to_string()
            };
            extra.insert(
                "build".to_string(),
                serde_json::Value::String(build_ctx),
            );
        }

        // Ports
        if let Some(ports) = value.get("ports").and_then(|p| p.as_sequence()) {
            let port_strs: Vec<serde_json::Value> = ports
                .iter()
                .filter_map(|p| p.as_str().map(|s| serde_json::Value::String(s.to_string())))
                .collect();
            if !port_strs.is_empty() {
                extra.insert("ports".to_string(), serde_json::Value::Array(port_strs));
            }
        }

        // Volumes
        if let Some(volumes) = value.get("volumes").and_then(|v| v.as_sequence()) {
            let vol_strs: Vec<serde_json::Value> = volumes
                .iter()
                .filter_map(|v| v.as_str().map(|s| serde_json::Value::String(s.to_string())))
                .collect();
            if !vol_strs.is_empty() {
                extra.insert("volumes".to_string(), serde_json::Value::Array(vol_strs));
            }
        }

        // Depends on
        if let Some(deps) = value.get("depends_on") {
            let dep_list: Vec<String> = if let Some(seq) = deps.as_sequence() {
                seq.iter()
                    .filter_map(|d| d.as_str().map(|s| s.to_string()))
                    .collect()
            } else if let Some(map) = deps.as_mapping() {
                map.keys()
                    .filter_map(|k| k.as_str().map(|s| s.to_string()))
                    .collect()
            } else {
                vec![]
            };
            if !dep_list.is_empty() {
                let dep_vals: Vec<serde_json::Value> = dep_list
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect();
                extra.insert("depends_on".to_string(), serde_json::Value::Array(dep_vals));
            }
        }

        // Compose file reference
        extra.insert(
            "compose_file".to_string(),
            serde_json::Value::String(compose_path.clone()),
        );

        service_resources.push(Resource {
            resource_type: "docker_service".to_string(),
            name: Some(service_name),
            path: Some(compose_path.clone()),
            provider: None,
            created: None,
            created_by: None,
            detected_by: "docker".to_string(),
            estimated_cost: None,
            extra,
        });
    }

    Ok((compose_resource, service_resources))
}
