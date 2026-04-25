use std::collections::HashMap;
use std::env;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use reqwest::{Client, StatusCode};

use crate::core::scanner::{ProviderTarget, ScanContext, ScanResult, Scanner};
use crate::core::types::{
    AuthMode, ContractCapability, ContractInfo, Provider, ProviderStatus, RateLimitInfo, Resource,
    SlaInfo,
};

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
                        contract: None,
                    });
                }
            }
        }

        Ok(ScanResult {
            resources: all_resources,
            providers: all_providers,
            resolution: None,
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

    // 3. Fetch the contract URL — api-key (header) or oauth2 (client credentials).
    let resp = if let Some(disc_url) = &pointer.discovery_url {
        match fetch_via_oauth2(client, &pointer.url, disc_url, &token).await {
            Some(r) => r,
            None => {
                provider.auth_error = Some("fetch_failed".to_string());
                return;
            }
        }
    } else {
        let header_name = pointer.header.as_deref().unwrap_or("X-Composit-Api-Key");
        match client
            .get(&pointer.url)
            .header(header_name, &token)
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => {
                provider.auth_error = Some("fetch_failed".to_string());
                return;
            }
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

    // 4. Parse the body against the RFC 003 v0.1 envelope. Unknown fields
    //    pass through silently (additionalProperties: true); missing or
    //    mismatched required fields fall back to public-tier with a
    //    specific error so `composit diff` can flag the shape problem
    //    separately from network/auth failures.
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => {
            provider.auth_error = Some("invalid_contract_body".to_string());
            return;
        }
    };

    match parse_contract_body(&body, &provider.name) {
        Some(info) => {
            provider.auth_mode = Some(AuthMode::Contract);
            provider.auth_error = None;
            provider.contract = Some(info);
        }
        None => {
            provider.auth_error = Some("invalid_contract_body".to_string());
        }
    }
}

/// Extract the `contract` bookkeeping subset from a RFC 003 response body.
///
/// Returns `None` when the response violates the v0.1 required-fields rule
/// (`contract.{id, provider, issued_at, expires_at}`) or when
/// `contract.provider` disagrees with the public manifest's provider name.
/// That guards against a misrouted contract URL silently accepting a wrong
/// provider's response.
fn parse_contract_body(body: &serde_json::Value, expected_provider: &str) -> Option<ContractInfo> {
    let contract = body.get("contract")?.as_object()?;
    let id = contract.get("id")?.as_str()?.to_string();
    let provider_field = contract.get("provider")?.as_str()?;
    if provider_field != expected_provider {
        return None;
    }
    let issued_at = contract.get("issued_at")?.as_str()?.to_string();
    let expires_at = contract.get("expires_at")?.as_str()?.to_string();
    let pricing_tier = contract
        .get("pricing_tier")
        .and_then(|v| v.as_str())
        .map(String::from);

    let sla = body.get("sla").and_then(|s| {
        let uptime_pct = s.get("uptime_pct").and_then(|v| v.as_f64());
        let incident_contact = s
            .get("incident_contact")
            .and_then(|v| v.as_str())
            .map(String::from);
        let response_time_ms_p99 = s.get("response_time_ms_p99").and_then(|v| v.as_u64());
        if uptime_pct.is_none() && incident_contact.is_none() && response_time_ms_p99.is_none() {
            None
        } else {
            Some(SlaInfo {
                uptime_pct,
                incident_contact,
                response_time_ms_p99,
            })
        }
    });

    let capabilities = body
        .get("capabilities")
        .and_then(|c| c.as_array())
        .map(|caps| {
            caps.iter()
                .filter_map(|cap| {
                    let cap_type = cap.get("type")?.as_str()?.to_string();
                    let rate_limit = cap.get("rate_limit").and_then(|rl| {
                        let rpm = rl.get("requests_per_minute").and_then(|v| v.as_u64());
                        let rph = rl.get("requests_per_hour").and_then(|v| v.as_u64());
                        let burst = rl.get("burst").and_then(|v| v.as_u64());
                        if rpm.is_none() && rph.is_none() && burst.is_none() {
                            None
                        } else {
                            Some(RateLimitInfo {
                                requests_per_minute: rpm,
                                requests_per_hour: rph,
                                burst,
                            })
                        }
                    });
                    Some(ContractCapability {
                        cap_type,
                        product: cap.get("product").and_then(|v| v.as_str()).map(String::from),
                        endpoint: cap.get("endpoint").and_then(|v| v.as_str()).map(String::from),
                        tools: cap.get("tools").and_then(|v| v.as_u64()),
                        rate_limit,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(ContractInfo {
        id,
        issued_at,
        expires_at,
        pricing_tier,
        sla,
        capabilities,
    })
}

#[derive(Debug, Clone)]
struct ContractPointer {
    url: String,
    /// api-key: name of the request header (default "X-Composit-Api-Key").
    header: Option<String>,
    /// oauth2: OIDC/AS discovery URL for client-credentials token fetch.
    discovery_url: Option<String>,
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
        let header = auth.get("header").and_then(|v| v.as_str()).map(String::from);
        let discovery_url = auth
            .get("discovery_url")
            .and_then(|v| v.as_str())
            .map(String::from);
        return Some(ContractPointer { url, header, discovery_url });
    }
    None
}

/// OAuth2 client-credentials flow (RFC 002 §Auth — roadmap).
///
/// `credentials` must be `"client_id:client_secret"`. The discovery document
/// at `discovery_url` (OIDC AS metadata or RFC 8414) must expose a
/// `token_endpoint`. Returns `None` on any network or parse failure so the
/// caller can fall back to `auth_error = "fetch_failed"`.
async fn fetch_via_oauth2(
    client: &Client,
    contract_url: &str,
    discovery_url: &str,
    credentials: &str,
) -> Option<reqwest::Response> {
    let (client_id, client_secret) = credentials.split_once(':').unwrap_or((credentials, ""));

    let discovery: serde_json::Value = client
        .get(discovery_url)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let token_endpoint = discovery.get("token_endpoint").and_then(|v| v.as_str())?;

    let token_resp: serde_json::Value = client
        .post(token_endpoint)
        .basic_auth(client_id, Some(client_secret))
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let access_token = token_resp.get("access_token").and_then(|v| v.as_str())?;

    client
        .get(contract_url)
        .bearer_auth(access_token)
        .send()
        .await
        .ok()
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
        contract: None,
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
        assert!(ptr.discovery_url.is_none(), "api-key pointer has no discovery_url");

        let oauth_ptr = find_contract_pointer(&manifest, "oauth2").expect("oauth2 matched");
        assert_eq!(oauth_ptr.url, "https://p.example.com/oauth-contract");
        assert_eq!(oauth_ptr.discovery_url.as_deref(), Some("https://..."));
        assert!(oauth_ptr.header.is_none(), "oauth2 pointer has no header");

        assert!(find_contract_pointer(&manifest, "mtls").is_none());
    }

    #[test]
    fn find_contract_pointer_none_when_no_contracts() {
        let manifest = json!({ "provider": { "name": "x" } });
        assert!(find_contract_pointer(&manifest, "api-key").is_none());
    }

    #[test]
    fn parse_contract_body_happy_path() {
        let body = json!({
            "composit": "0.1.0",
            "contract": {
                "id": "c-2026-nuetzliche-42",
                "provider": "nuetzliche",
                "issued_at": "2026-04-01T00:00:00Z",
                "expires_at": "2027-04-01T00:00:00Z",
                "pricing_tier": "team"
            },
            "capabilities": []
        });

        let info = parse_contract_body(&body, "nuetzliche").expect("happy path parses");
        assert_eq!(info.id, "c-2026-nuetzliche-42");
        assert_eq!(info.issued_at, "2026-04-01T00:00:00Z");
        assert_eq!(info.expires_at, "2027-04-01T00:00:00Z");
        assert_eq!(info.pricing_tier.as_deref(), Some("team"));
        assert!(info.sla.is_none());
        assert!(info.capabilities.is_empty());
    }

    #[test]
    fn parse_contract_body_extracts_sla_and_capabilities() {
        let body = json!({
            "composit": "0.1.0",
            "contract": {
                "id": "c-1",
                "provider": "nuetzliche",
                "issued_at": "2026-01-01T00:00:00Z",
                "expires_at": "2027-01-01T00:00:00Z"
            },
            "capabilities": [
                {
                    "type": "scheduling",
                    "product": "croniq",
                    "endpoint": "https://mcp.nuetzliche.it/croniq",
                    "tools": 12,
                    "rate_limit": { "requests_per_minute": 120, "burst": 20 }
                },
                {
                    "type": "events",
                    "product": "hookaido"
                }
            ],
            "sla": {
                "uptime_pct": 99.5,
                "incident_contact": "sre@nuetzliche.it",
                "response_time_ms_p99": 800
            }
        });

        let info = parse_contract_body(&body, "nuetzliche").expect("parses");

        let sla = info.sla.expect("sla present");
        assert_eq!(sla.uptime_pct, Some(99.5));
        assert_eq!(sla.incident_contact.as_deref(), Some("sre@nuetzliche.it"));
        assert_eq!(sla.response_time_ms_p99, Some(800));

        assert_eq!(info.capabilities.len(), 2);
        let croniq = &info.capabilities[0];
        assert_eq!(croniq.cap_type, "scheduling");
        assert_eq!(croniq.product.as_deref(), Some("croniq"));
        assert_eq!(croniq.endpoint.as_deref(), Some("https://mcp.nuetzliche.it/croniq"));
        assert_eq!(croniq.tools, Some(12));
        let rl = croniq.rate_limit.as_ref().expect("rate_limit parsed");
        assert_eq!(rl.requests_per_minute, Some(120));
        assert_eq!(rl.burst, Some(20));
        assert!(rl.requests_per_hour.is_none());

        // Sparse capability: just type + product, no endpoint/tools/rate_limit.
        let hookaido = &info.capabilities[1];
        assert_eq!(hookaido.cap_type, "events");
        assert!(hookaido.endpoint.is_none());
        assert!(hookaido.tools.is_none());
        assert!(hookaido.rate_limit.is_none());
    }

    #[test]
    fn parse_contract_body_empty_sla_block_is_none() {
        // RFC 003 §sla: all sub-fields optional; empty `sla: {}` should
        // collapse to `None` rather than carry a meaningless object.
        let body = json!({
            "contract": {
                "id": "c-1",
                "provider": "x",
                "issued_at": "2026-01-01T00:00:00Z",
                "expires_at": "2027-01-01T00:00:00Z"
            },
            "sla": {}
        });
        let info = parse_contract_body(&body, "x").expect("parses");
        assert!(info.sla.is_none(), "empty sla object collapses to None");
    }

    #[test]
    fn parse_contract_body_missing_required_field_returns_none() {
        // expires_at absent.
        let body = json!({
            "contract": {
                "id": "c-1",
                "provider": "x",
                "issued_at": "2026-01-01T00:00:00Z"
            }
        });
        assert!(parse_contract_body(&body, "x").is_none());
    }

    #[test]
    fn parse_contract_body_rejects_provider_mismatch() {
        // contract.provider disagrees with the public manifest's
        // provider.name — guards against a misrouted contract URL.
        let body = json!({
            "contract": {
                "id": "c-1",
                "provider": "acme",
                "issued_at": "2026-01-01T00:00:00Z",
                "expires_at": "2027-01-01T00:00:00Z"
            }
        });
        assert!(parse_contract_body(&body, "nuetzliche").is_none());
    }

    #[test]
    fn parse_contract_body_tolerates_unknown_fields() {
        // RFC 003 §Unknown fields: additionalProperties: true at every
        // level — providers MAY embed vendor-specific keys.
        let body = json!({
            "contract": {
                "id": "c-1",
                "provider": "x",
                "issued_at": "2026-01-01T00:00:00Z",
                "expires_at": "2027-01-01T00:00:00Z",
                "x-internal-seq": 42
            },
            "sla": {"uptime_pct": 99.9},
            "x-region-overrides": {"eu-west-1": true}
        });
        let info = parse_contract_body(&body, "x").expect("unknown fields ignored");
        assert_eq!(info.id, "c-1");
        assert!(info.pricing_tier.is_none());
    }
}
