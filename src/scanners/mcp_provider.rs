use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::{Provider, ProviderStatus, Resource};

pub struct McpProviderScanner;

#[async_trait]
impl Scanner for McpProviderScanner {
    fn id(&self) -> &str {
        "mcp_provider"
    }

    fn name(&self) -> &str {
        "MCP Provider Scanner"
    }

    fn description(&self) -> &str {
        "Connects to MCP providers via /.well-known/composit.json"
    }

    fn needs_network(&self) -> bool {
        true
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

        let mut all_resources = Vec::new();
        let mut all_providers = Vec::new();

        for url in &context.providers {
            match fetch_provider(&client, url).await {
                Ok((provider, resources)) => {
                    all_providers.push(provider);
                    all_resources.extend(resources);
                }
                Err(e) => {
                    eprintln!("Warning: could not reach provider {}: {}", url, e);
                    all_providers.push(Provider {
                        name: url.clone(),
                        endpoint: url.clone(),
                        protocol: "unknown".to_string(),
                        capabilities: vec![],
                        status: ProviderStatus::Unreachable,
                    });
                }
            }
        }

        Ok(ScanResult {
            resources: all_resources,
            providers: all_providers,
        })
    }
}

/// Fetch a provider's composit manifest and extract capabilities
async fn fetch_provider(client: &Client, base_url: &str) -> Result<(Provider, Vec<Resource>)> {
    let manifest_url = format!(
        "{}/.well-known/composit.json",
        base_url.trim_end_matches('/')
    );

    let resp = client.get(&manifest_url).send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }

    let manifest: serde_json::Value = resp.json().await?;
    Ok(parse_manifest(&manifest, base_url))
}

/// Pure function: given a parsed manifest and the base URL, return the
/// derived Provider + capability-level Resources. Separated from the HTTP
/// fetch so it can be unit-tested.
fn parse_manifest(manifest: &serde_json::Value, base_url: &str) -> (Provider, Vec<Resource>) {
    let provider_name = manifest
        .get("provider")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown")
        .to_string();

    let mut capabilities = Vec::new();
    let mut resources = Vec::new();

    if let Some(caps) = manifest.get("capabilities").and_then(|c| c.as_array()) {
        for cap in caps {
            if let Some(cap_type) = cap.get("type").and_then(|t| t.as_str()) {
                capabilities.push(cap_type.to_string());
            }

            if let Some(product) = cap.get("product").and_then(|p| p.as_str()) {
                let mut extra = HashMap::new();
                if let Some(tools) = cap.get("tools").and_then(|t| t.as_u64()) {
                    extra.insert(
                        "tools".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(tools)),
                    );
                }
                if let Some(desc) = cap.get("description").and_then(|d| d.as_str()) {
                    extra.insert(
                        "description".to_string(),
                        serde_json::Value::String(desc.to_string()),
                    );
                }

                resources.push(Resource {
                    resource_type: "mcp_capability".to_string(),
                    name: Some(product.to_string()),
                    path: None,
                    provider: Some(provider_name.clone()),
                    created: None,
                    created_by: None,
                    detected_by: "mcp_provider".to_string(),
                    estimated_cost: None,
                    extra,
                });
            }
        }
    }

    let provider = Provider {
        name: provider_name,
        endpoint: base_url.to_string(),
        protocol: "mcp".to_string(),
        capabilities,
        status: ProviderStatus::Reachable,
    };

    (provider, resources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_manifest_full_capabilities() {
        let manifest = json!({
            "provider": { "name": "croniq" },
            "capabilities": [
                {
                    "type": "scheduling",
                    "product": "croniq",
                    "tools": 12,
                    "description": "Cron-as-a-service"
                },
                {
                    "type": "events"
                }
            ]
        });

        let (provider, resources) = parse_manifest(&manifest, "https://composit.example.com/");
        assert_eq!(provider.name, "croniq");
        assert_eq!(provider.protocol, "mcp");
        assert_eq!(provider.capabilities, vec!["scheduling", "events"]);

        assert_eq!(resources.len(), 1);
        let r = &resources[0];
        assert_eq!(r.resource_type, "mcp_capability");
        assert_eq!(r.name.as_deref(), Some("croniq"));
        assert_eq!(r.provider.as_deref(), Some("croniq"));
        assert_eq!(r.extra.get("tools").and_then(|v| v.as_u64()), Some(12));
        assert_eq!(
            r.extra.get("description").and_then(|v| v.as_str()),
            Some("Cron-as-a-service")
        );
    }

    #[test]
    fn test_parse_manifest_unknown_provider_name() {
        let manifest = json!({ "capabilities": [] });
        let (provider, resources) = parse_manifest(&manifest, "https://example.com");
        assert_eq!(provider.name, "unknown");
        assert!(resources.is_empty());
        assert!(provider.capabilities.is_empty());
    }

    #[test]
    fn test_parse_manifest_empty() {
        let manifest = json!({});
        let (provider, resources) = parse_manifest(&manifest, "https://example.com");
        assert_eq!(provider.name, "unknown");
        assert!(resources.is_empty());
    }
}
