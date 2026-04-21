use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;
use hcl::Body;

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
        "Detects Terraform configurations, resources, and modules"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        // Phase 1: Parse .tf files grouped by directory
        let tf_pattern = context.dir.join("**/*.tf");
        let mut tf_files_by_dir: HashMap<String, Vec<std::path::PathBuf>> = HashMap::new();
        for path in glob(&tf_pattern.to_string_lossy())?.flatten() {
            if context.is_excluded(&path) {
                continue;
            }
            if let Some(parent) = path.parent() {
                let rel_dir = parent
                    .strip_prefix(&context.dir)
                    .unwrap_or(parent)
                    .to_string_lossy()
                    .to_string();
                tf_files_by_dir.entry(rel_dir).or_default().push(path);
            }
        }

        for (dir, files) in &tf_files_by_dir {
            let (config_resource, nested_resources) = scan_tf_directory(dir, files, &context.dir);
            resources.push(config_resource);
            resources.extend(nested_resources);
        }

        // Phase 2: Scan for .tfstate files (enrichment)
        let state_pattern = context.dir.join("**/*.tfstate");
        for path in glob(&state_pattern.to_string_lossy())?.flatten() {
            if context.is_excluded(&path) {
                continue;
            }
            let rel_path = path
                .strip_prefix(&context.dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

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

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}

/// Parsed block counts and nested resources from a Terraform directory.
struct TfDirSummary {
    resource_count: usize,
    module_count: usize,
    variable_count: usize,
    output_count: usize,
    providers: Vec<String>,
    nested_resources: Vec<Resource>,
}

/// Parse all .tf files in a directory, extract blocks, and create resources.
fn scan_tf_directory(
    dir: &str,
    files: &[std::path::PathBuf],
    base_dir: &Path,
) -> (Resource, Vec<Resource>) {
    let display_dir = if dir.is_empty() {
        ".".to_string()
    } else {
        format!("./{}", dir)
    };

    let mut summary = TfDirSummary {
        resource_count: 0,
        module_count: 0,
        variable_count: 0,
        output_count: 0,
        providers: Vec::new(),
        nested_resources: Vec::new(),
    };

    for file_path in files {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "warning: terraform scanner could not read {}: {}",
                    file_path.display(),
                    e
                );
                continue;
            }
        };

        let body: Body = match hcl::from_str(&content) {
            Ok(b) => b,
            Err(e) => {
                eprintln!(
                    "warning: terraform scanner could not parse {}: {}",
                    file_path.display(),
                    e
                );
                continue;
            }
        };

        let rel_path = file_path
            .strip_prefix(base_dir)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();
        let file_display = format!("./{}", rel_path);

        parse_tf_body(&body, &file_display, &display_dir, &mut summary);
    }

    // Build aggregated terraform_config resource
    let mut config_extra = HashMap::new();
    config_extra.insert(
        "resources".to_string(),
        serde_json::Value::Number(serde_json::Number::from(summary.resource_count)),
    );
    config_extra.insert(
        "modules".to_string(),
        serde_json::Value::Number(serde_json::Number::from(summary.module_count)),
    );
    config_extra.insert(
        "variables".to_string(),
        serde_json::Value::Number(serde_json::Number::from(summary.variable_count)),
    );
    config_extra.insert(
        "outputs".to_string(),
        serde_json::Value::Number(serde_json::Number::from(summary.output_count)),
    );

    // Deduplicate providers
    summary.providers.sort();
    summary.providers.dedup();
    if !summary.providers.is_empty() {
        config_extra.insert(
            "provider_list".to_string(),
            serde_json::Value::Array(
                summary
                    .providers
                    .iter()
                    .map(|p| serde_json::Value::String(p.clone()))
                    .collect(),
            ),
        );
    }

    let config_resource = Resource {
        resource_type: "terraform_config".to_string(),
        name: None,
        path: Some(display_dir),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "terraform".to_string(),
        estimated_cost: None,
        extra: config_extra,
    };

    (config_resource, summary.nested_resources)
}

/// Parse an HCL body and extract resource, module, provider, variable, output blocks.
fn parse_tf_body(body: &Body, file_path: &str, tf_dir: &str, summary: &mut TfDirSummary) {
    for block in body.blocks() {
        match block.identifier.as_str() {
            "resource" => {
                summary.resource_count += 1;
                let labels: Vec<&str> = block.labels.iter().map(|l| l.as_str()).collect();
                let resource_type = labels.first().copied().unwrap_or("unknown");
                let resource_name = labels.get(1).copied().unwrap_or("unnamed");

                let mut extra = HashMap::new();
                extra.insert(
                    "resource_type".to_string(),
                    serde_json::Value::String(resource_type.to_string()),
                );
                extra.insert(
                    "resource_name".to_string(),
                    serde_json::Value::String(resource_name.to_string()),
                );
                extra.insert(
                    "tf_dir".to_string(),
                    serde_json::Value::String(tf_dir.to_string()),
                );

                summary.nested_resources.push(Resource {
                    resource_type: "terraform_resource".to_string(),
                    name: Some(format!("{}.{}", resource_type, resource_name)),
                    path: Some(file_path.to_string()),
                    provider: None,
                    created: None,
                    created_by: None,
                    detected_by: "terraform".to_string(),
                    estimated_cost: None,
                    extra,
                });
            }
            "module" => {
                summary.module_count += 1;
                let module_name = block
                    .labels
                    .first()
                    .map(|l| l.as_str())
                    .unwrap_or("unnamed");

                let mut extra = HashMap::new();
                extra.insert(
                    "tf_dir".to_string(),
                    serde_json::Value::String(tf_dir.to_string()),
                );

                // Extract source attribute
                if let Some(source) = get_string_attr(&block.body, "source") {
                    extra.insert("source".to_string(), serde_json::Value::String(source));
                }

                // Extract version attribute
                if let Some(version) = get_string_attr(&block.body, "version") {
                    extra.insert("version".to_string(), serde_json::Value::String(version));
                }

                summary.nested_resources.push(Resource {
                    resource_type: "terraform_module".to_string(),
                    name: Some(module_name.to_string()),
                    path: Some(file_path.to_string()),
                    provider: None,
                    created: None,
                    created_by: None,
                    detected_by: "terraform".to_string(),
                    estimated_cost: None,
                    extra,
                });
            }
            "provider" => {
                let provider_name = block
                    .labels
                    .first()
                    .map(|l| l.as_str().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                summary.providers.push(provider_name);
            }
            "variable" => {
                summary.variable_count += 1;
            }
            "output" => {
                summary.output_count += 1;
            }
            _ => {}
        }
    }
}

/// Extract a string attribute from an HCL body.
fn get_string_attr(body: &Body, key: &str) -> Option<String> {
    body.attributes()
        .find(|a| a.key.as_str() == key)
        .and_then(|a| match &a.expr {
            hcl::Expression::String(s) => Some(s.clone()),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_hcl_to_summary(hcl_content: &str) -> TfDirSummary {
        let body: Body = hcl::from_str(hcl_content).unwrap();
        let mut summary = TfDirSummary {
            resource_count: 0,
            module_count: 0,
            variable_count: 0,
            output_count: 0,
            providers: Vec::new(),
            nested_resources: Vec::new(),
        };
        parse_tf_body(&body, "./main.tf", "./infra", &mut summary);
        summary
    }

    #[test]
    fn test_parse_resources() {
        let summary = parse_hcl_to_summary(
            r#"
            resource "aws_instance" "web" {
              ami           = "ami-abc123"
              instance_type = "t2.micro"
            }

            resource "aws_s3_bucket" "data" {
              bucket = "my-data-bucket"
            }
            "#,
        );

        assert_eq!(summary.resource_count, 2);
        assert_eq!(summary.nested_resources.len(), 2);

        let first = &summary.nested_resources[0];
        assert_eq!(first.resource_type, "terraform_resource");
        assert_eq!(first.name.as_deref(), Some("aws_instance.web"));
        assert_eq!(
            first.extra.get("resource_type").and_then(|v| v.as_str()),
            Some("aws_instance")
        );
        assert_eq!(
            first.extra.get("resource_name").and_then(|v| v.as_str()),
            Some("web")
        );
    }

    #[test]
    fn test_parse_modules() {
        let summary = parse_hcl_to_summary(
            r#"
            module "vpc" {
              source  = "terraform-aws-modules/vpc/aws"
              version = "5.0.0"
              cidr    = "10.0.0.0/16"
            }
            "#,
        );

        assert_eq!(summary.module_count, 1);
        assert_eq!(summary.nested_resources.len(), 1);

        let module = &summary.nested_resources[0];
        assert_eq!(module.resource_type, "terraform_module");
        assert_eq!(module.name.as_deref(), Some("vpc"));
        assert_eq!(
            module.extra.get("source").and_then(|v| v.as_str()),
            Some("terraform-aws-modules/vpc/aws")
        );
        assert_eq!(
            module.extra.get("version").and_then(|v| v.as_str()),
            Some("5.0.0")
        );
    }

    #[test]
    fn test_parse_providers_and_variables() {
        let summary = parse_hcl_to_summary(
            r#"
            provider "aws" {
              region = "eu-central-1"
            }

            provider "cloudflare" {
              api_token = var.cf_token
            }

            variable "region" {
              type    = string
              default = "eu-central-1"
            }

            variable "instance_count" {
              type = number
            }

            output "vpc_id" {
              value = module.vpc.id
            }
            "#,
        );

        assert_eq!(summary.providers.len(), 2);
        assert!(summary.providers.contains(&"aws".to_string()));
        assert!(summary.providers.contains(&"cloudflare".to_string()));
        assert_eq!(summary.variable_count, 2);
        assert_eq!(summary.output_count, 1);
        // Providers and variables don't create nested resources
        assert_eq!(summary.nested_resources.len(), 0);
    }

    #[test]
    fn test_empty_and_invalid_hcl() {
        // Empty body
        let summary = parse_hcl_to_summary("");
        assert_eq!(summary.resource_count, 0);
        assert_eq!(summary.nested_resources.len(), 0);

        // Invalid HCL should not panic (tested via scan_tf_directory which skips errors)
    }

    #[test]
    fn test_mixed_config() {
        let summary = parse_hcl_to_summary(
            r#"
            terraform {
              required_version = ">= 1.0"
              required_providers {
                aws = {
                  source  = "hashicorp/aws"
                  version = "~> 5.0"
                }
              }
            }

            provider "aws" {
              region = "eu-central-1"
            }

            resource "aws_vpc" "main" {
              cidr_block = "10.0.0.0/16"
            }

            resource "aws_subnet" "public" {
              vpc_id     = aws_vpc.main.id
              cidr_block = "10.0.1.0/24"
            }

            module "eks" {
              source  = "terraform-aws-modules/eks/aws"
              version = "20.0.0"
            }

            variable "environment" {
              type    = string
              default = "production"
            }

            output "vpc_id" {
              value = aws_vpc.main.id
            }
            "#,
        );

        assert_eq!(summary.resource_count, 2);
        assert_eq!(summary.module_count, 1);
        assert_eq!(summary.variable_count, 1);
        assert_eq!(summary.output_count, 1);
        assert_eq!(summary.providers, vec!["aws".to_string()]);
        assert_eq!(summary.nested_resources.len(), 3); // 2 resources + 1 module
    }
}
