use std::collections::HashMap;
use std::env;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use reqwest::{Client, StatusCode};

use crate::core::scanner::{ProviderTarget, ScanContext, ScanResult, Scanner};
use crate::core::types::{AuthMode, Provider, ProviderStatus, Resource};

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
        "Connects to MCP providers via /.well-known/composit.json and, \
         when the Compositfile declares trust=contract, also fetches the \
         contract manifest"
    }

    fn needs_network(&self) -> bool {
        true
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

        let mut all_resources = Vec::new();
        let mut all_providers = Vec::new();

        for target in &context.providers {
            match fetch_public(&client, &target.url).await {
                Ok((mut provider, resources)) => {
                    provider.auth_mode = Some(AuthMode::Public);
                    all_resources.extend(resources);

                    // If governance asked for contract-tier, try to upgrade.
                    if target.trust.as_deref() == Some("contract") {
                        upgrade_to_contract(&client, &mut provider, target).await;
                    }

                    all_providers.push(provider);
                }
                Err(e) => {
                    eprintln!("Warning: could not reach provider {}: {}", target.url, e);
                    all_providers.push(Provider {
                        name: target.url.clone(),
                        endpoint: target.url.clone(),
                        protocol: "unknown".to_string(),
                        capabilities: vec![],
                        status: ProviderStatus::Unreachable,
                        auth_mode: Some(AuthMode::Unreachable),
                        auth_error: Some("fetch_failed".to_string()),
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

/// Fetch a provider's public manifest and extract capabilities.
/// The returned Provider has `auth_mode = None` — the scan loop sets it
/// based on what happens next.
async fn fetch_public(client: &Client, base_url: &str) -> Result<(Provider, Vec<Resource>)> {
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

/// Attempt a contract-tier fetch. Mutates the Provider in place to record
/// either success (auth_mode = Contract) or a specific failure reason
/// (auth_error = ...). Never returns an error — the scan continues even
/// when the contract path fails; `composit diff` reports on the outcome.
async fn upgrade_to_contract(client: &Client, provider: &mut Provider, target: &ProviderTarget) {
    // 1. Credential present?
    let env_name = match &target.auth_env {
        Some(n) => n,
        None => {
            provider.auth_error = Some("auth_missing".to_string());
            return;
        }
    };
    let token = match env::var(env_name) {
        Ok(v) if !v.is_empty() => v,
        _ => {
            provider.auth_error = Some("auth_missing".to_string());
            return;
        }
    };

    // 2. Public manifest must have advertised a matching contract endpoint.
    //    We refetch the public manifest here (small cost) to avoid plumbing
    //    the raw JSON through fetch_public's return — the pure parser
    //    function stays compatible with existing tests.
    let public_url = format!(
        "{}/.well-known/composit.json",
        provider.endpoint.trim_end_matches('/')
    );
    let manifest: serde_json::Value = match client.get(&public_url).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json().await {
            Ok(v) => v,
            Err(_) => {
                provider.auth_error = Some("fetch_failed".to_string());
                return;
            }
        },
        _ => {
            provider.auth_error = Some("fetch_failed".to_string());
            return;
        }
    };

    let want_type = target.auth_type.as_deref().unwrap_or("api-key");
    let pointer = match find_contract_pointer(&manifest, want_type) {
        Some(p) => p,
        None => {
            provider.auth_error = Some("auth_type_not_advertised".to_string());
            return;
        }
    };

    // 3. Fetch the contract URL with the credential.
    let header_name = pointer.header.as_deref().unwrap_or("X-Composit-Api-Key");
    let resp = client
        .get(&pointer.url)
        .header(header_name, &token)
        .send()
        .await;
    let resp = match resp {
        Ok(r) => r,
        Err(_) => {
            provider.auth_error = Some("fetch_failed".to_string());
            return;
        }
    };

    let status = resp.status();
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        provider.auth_error = Some("unauthorized".to_string());
        return;
    }
    if !status.is_success() {
        provider.auth_error = Some("fetch_failed".to_string());
        return;
    }

    // 4. Success. v0.1 doesn't parse the contract body into Provider
    //    fields yet — schema is still pending in RFC 003. We mark the
    //    mode and clear any prior error so `composit diff` sees a clean
    //    contract state.
    provider.auth_mode = Some(AuthMode::Contract);
    provider.auth_error = None;
}

#[derive(Debug, Clone)]
struct ContractPointer {
    url: String,
    header: Option<String>,
}

/// Find the first `contracts[]` entry whose `auth.type` matches `want_type`.
/// Public manifest shape per RFC 002 / v0.1 schema.
fn find_contract_pointer(manifest: &serde_json::Value, want_type: &str) -> Option<ContractPointer> {
    let contracts = manifest.get("contracts")?.as_array()?;
    for entry in contracts {
        let auth = entry.get("auth")?;
        let ty = auth.get("type").and_then(|v| v.as_str());
        if ty != Some(want_type) {
            continue;
        }
        let url = entry.get("url").and_then(|v| v.as_str())?.to_string();
        let header = auth
            .get("header")
            .and_then(|v| v.as_str())
            .map(String::from);
        return Some(ContractPointer { url, header });
    }
    None
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
        // Scan loop will set auth_mode once the public-vs-contract path is
        // decided; leave as None here so the parser stays pure.
        auth_mode: None,
        auth_error: None,
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
        assert!(provider.auth_mode.is_none());
        assert!(provider.auth_error.is_none());

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

    #[test]
    fn find_contract_pointer_picks_matching_auth_type() {
        let manifest = json!({
            "contracts": [
                {
                    "url": "https://p.example.com/oauth-contract",
                    "auth": { "type": "oauth2", "discovery_url": "https://..." }
                },
                {
                    "url": "https://p.example.com/contract",
                    "auth": { "type": "api-key", "header": "X-Custom-Key" }
                }
            ]
        });

        let ptr = find_contract_pointer(&manifest, "api-key").expect("api-key matched");
        assert_eq!(ptr.url, "https://p.example.com/contract");
        assert_eq!(ptr.header.as_deref(), Some("X-Custom-Key"));

        assert!(find_contract_pointer(&manifest, "mtls").is_none());
    }

    #[test]
    fn find_contract_pointer_none_when_no_contracts() {
        let manifest = json!({ "provider": { "name": "x" } });
        assert!(find_contract_pointer(&manifest, "api-key").is_none());
    }
}
