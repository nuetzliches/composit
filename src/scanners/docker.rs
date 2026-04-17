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
                let Ok(path) = entry else { continue };
                match scan_compose_file(&path, &context.dir) {
                    Ok((compose_resource, service_resources)) => {
                        resources.push(compose_resource);
                        resources.extend(service_resources);
                    }
                    Err(e) => {
                        eprintln!(
                            "warning: docker scanner failed on {}: {}",
                            path.display(),
                            e
                        );
                    }
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

    // Extract top-level networks
    let networks: Vec<String> = yaml
        .get("networks")
        .and_then(|n| n.as_mapping())
        .map(|m| {
            m.keys()
                .filter_map(|k| k.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // Extract top-level volumes
    let volumes: Vec<String> = yaml
        .get("volumes")
        .and_then(|v| v.as_mapping())
        .map(|m| {
            m.keys()
                .filter_map(|k| k.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // Compose file resource
    let mut compose_extra = HashMap::new();
    compose_extra.insert(
        "services".to_string(),
        serde_json::Value::Number(serde_json::Number::from(services_count)),
    );
    if !networks.is_empty() {
        compose_extra.insert(
            "networks".to_string(),
            serde_json::Value::Array(
                networks
                    .iter()
                    .map(|n| serde_json::Value::String(n.clone()))
                    .collect(),
            ),
        );
    }
    if !volumes.is_empty() {
        compose_extra.insert(
            "volumes".to_string(),
            serde_json::Value::Array(
                volumes
                    .iter()
                    .map(|v| serde_json::Value::String(v.clone()))
                    .collect(),
            ),
        );
    }

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

        // Service networks
        if let Some(nets) = value.get("networks") {
            let net_list: Vec<String> = if let Some(seq) = nets.as_sequence() {
                seq.iter()
                    .filter_map(|n| n.as_str().map(|s| s.to_string()))
                    .collect()
            } else if let Some(map) = nets.as_mapping() {
                map.keys()
                    .filter_map(|k| k.as_str().map(|s| s.to_string()))
                    .collect()
            } else {
                vec![]
            };
            if !net_list.is_empty() {
                let net_vals: Vec<serde_json::Value> = net_list
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect();
                extra.insert("networks".to_string(), serde_json::Value::Array(net_vals));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn test_scan_compose_file_extracts_services_networks_volumes() {
        let dir = tempdir().unwrap();
        let compose = r#"
services:
  api:
    image: ghcr.io/acme/api:1.2.3
    ports:
      - "8080:80"
    volumes:
      - api_data:/var/lib/api
    networks:
      - internal
  db:
    image: postgres:16
    volumes:
      - db_data:/var/lib/postgresql/data

networks:
  internal:

volumes:
  api_data:
  db_data:
"#;
        write(dir.path(), "docker-compose.yml", compose);

        let (compose_res, services) =
            scan_compose_file(&dir.path().join("docker-compose.yml"), dir.path()).unwrap();

        assert_eq!(compose_res.resource_type, "docker_compose");
        assert_eq!(services.len(), 2);

        let api = services
            .iter()
            .find(|r| r.name.as_deref() == Some("api"))
            .expect("api service present");
        assert_eq!(
            api.extra.get("image").and_then(|v| v.as_str()),
            Some("ghcr.io/acme/api:1.2.3")
        );
    }

    #[test]
    fn test_malformed_compose_yields_error() {
        let dir = tempdir().unwrap();
        write(dir.path(), "docker-compose.yml", "services:\n  api:\n    image: [broken");
        let result =
            scan_compose_file(&dir.path().join("docker-compose.yml"), dir.path());
        assert!(result.is_err(), "broken YAML must error so caller can warn");
    }

    #[test]
    fn test_empty_services_block() {
        let dir = tempdir().unwrap();
        write(dir.path(), "docker-compose.yml", "version: \"3\"\nservices: {}\n");
        let (compose_res, services) =
            scan_compose_file(&dir.path().join("docker-compose.yml"), dir.path()).unwrap();
        assert_eq!(compose_res.resource_type, "docker_compose");
        assert!(services.is_empty());
    }
}
