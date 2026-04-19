use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::Client;

use crate::core::types::{Provider, ProviderStatus, Report};

/// Live manifest info merged onto a Provider for display-only purposes.
/// Not part of the persisted Report — `status --live` is a transient view.
#[derive(Debug, Default, Clone)]
struct LiveInfo {
    description: Option<String>,
    region: Option<String>,
    compliance: Vec<String>,
}

/// Load the report from disk and display aggregated status
pub async fn run_status(dir: &Path, live: bool) -> Result<()> {
    let report_path = dir.join("composit-report.yaml");
    if !report_path.exists() {
        anyhow::bail!(
            "No composit-report.yaml found in {}. Run `composit scan` first.",
            dir.display()
        );
    }

    let content = std::fs::read_to_string(&report_path)
        .with_context(|| format!("Failed to read {}", report_path.display()))?;
    let mut report: Report =
        serde_yaml::from_str(&content).with_context(|| "Failed to parse report")?;

    // Live provider checks: fetch and parse each manifest, merge findings
    // back onto the in-memory provider list for display.
    let mut live_info: HashMap<String, LiveInfo> = HashMap::new();
    if live && !report.providers.is_empty() {
        live_info = check_providers_live(&mut report.providers).await?;
    }

    print_status(&report, &live_info);

    Ok(())
}

async fn check_providers_live(providers: &mut [Provider]) -> Result<HashMap<String, LiveInfo>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("Failed to build HTTP client for live provider checks")?;

    let mut live_info = HashMap::new();

    for provider in providers.iter_mut() {
        let url = format!(
            "{}/.well-known/composit.json",
            provider.endpoint.trim_end_matches('/')
        );

        let Ok(resp) = client.get(&url).send().await else {
            provider.status = ProviderStatus::Unreachable;
            continue;
        };

        if !resp.status().is_success() {
            provider.status = ProviderStatus::Unreachable;
            continue;
        }

        // Reachable and returned 2xx — now try to parse.
        let Ok(manifest) = resp.json::<serde_json::Value>().await else {
            // HTTP OK but body is not valid JSON. Count as reachable
            // (endpoint exists) but with no enriched info.
            provider.status = ProviderStatus::Reachable;
            continue;
        };

        provider.status = ProviderStatus::Reachable;
        let info = merge_manifest_into_provider(&manifest, provider);
        live_info.insert(provider.name.clone(), info);
    }

    Ok(live_info)
}

/// Enrich the in-memory Provider with fields read from the manifest.
/// Returns the extra display-only info (description, compliance, region)
/// that doesn't have a home on the persisted Provider struct.
fn merge_manifest_into_provider(manifest: &serde_json::Value, provider: &mut Provider) -> LiveInfo {
    // If the manifest declares a canonical name, prefer it.
    if let Some(name) = manifest
        .get("provider")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
    {
        if !name.is_empty() {
            provider.name = name.to_string();
        }
    }

    // Replace capabilities with what the provider actually advertises.
    if let Some(caps) = manifest.get("capabilities").and_then(|c| c.as_array()) {
        let advertised: Vec<String> = caps
            .iter()
            .filter_map(|c| c.get("type").and_then(|t| t.as_str()).map(String::from))
            .collect();
        if !advertised.is_empty() {
            provider.capabilities = advertised;
        }

        // If every advertised capability agrees on a protocol, promote it.
        let protocols: std::collections::HashSet<&str> = caps
            .iter()
            .filter_map(|c| c.get("protocol").and_then(|p| p.as_str()))
            .collect();
        if protocols.len() == 1 {
            if let Some(p) = protocols.into_iter().next() {
                provider.protocol = p.to_string();
            }
        }
    }

    LiveInfo {
        description: manifest
            .get("provider")
            .and_then(|p| p.get("description"))
            .and_then(|d| d.as_str())
            .map(String::from),
        region: manifest
            .get("region")
            .and_then(|r| r.as_str())
            .map(String::from),
        compliance: manifest
            .get("compliance")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn print_status(report: &Report, live_info: &HashMap<String, LiveInfo>) {
    println!();
    println!("{}", "composit status".bold());
    println!("{} {}", "Workspace:".dimmed(), report.workspace.bold());
    println!("{} {}", "Last scan:".dimmed(), report.generated);
    println!("{}", "=".repeat(60));

    // Resources by type
    let mut by_type: HashMap<&str, usize> = HashMap::new();
    for r in &report.resources {
        *by_type.entry(&r.resource_type).or_insert(0) += 1;
    }

    println!();
    println!("  {}", "Resources".bold());
    let mut types: Vec<_> = by_type.iter().collect();
    types.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    for (resource_type, count) in &types {
        println!("    {:30} {}", resource_type, count.to_string().cyan());
    }

    // Attribution breakdown
    println!();
    println!("  {}", "Attribution".bold());

    let mut by_author: HashMap<&str, usize> = HashMap::new();
    let mut untracked = 0usize;
    for r in &report.resources {
        match &r.created_by {
            Some(author) => *by_author.entry(author).or_insert(0) += 1,
            None => untracked += 1,
        }
    }

    let mut authors: Vec<_> = by_author.iter().collect();
    authors.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    for (author, count) in &authors {
        let display = if author.starts_with("agent:") {
            author.yellow().to_string()
        } else {
            author.to_string()
        };
        println!("    {:30} {}", display, count);
    }
    if untracked > 0 {
        println!("    {:30} {}", "untracked".dimmed(), untracked);
    }

    // Providers
    if !report.providers.is_empty() {
        println!();
        println!("  {}", "Providers".bold());
        for p in &report.providers {
            let status = match p.status {
                ProviderStatus::Reachable => "reachable".green(),
                ProviderStatus::Unreachable => "unreachable".red(),
                ProviderStatus::Unknown => "unknown".yellow(),
            };
            let caps = if p.capabilities.is_empty() {
                String::new()
            } else {
                format!(" ({})", p.capabilities.join(", "))
                    .dimmed()
                    .to_string()
            };
            println!("    {:30} {}{}", p.name, status, caps);

            // Live manifest details (description, region, compliance)
            if let Some(info) = live_info.get(&p.name) {
                if let Some(desc) = &info.description {
                    println!("      {} {}", "·".dimmed(), desc.dimmed());
                }
                if let Some(region) = &info.region {
                    println!("      {} region: {}", "·".dimmed(), region.cyan());
                }
                if !info.compliance.is_empty() {
                    println!(
                        "      {} compliance: {}",
                        "·".dimmed(),
                        info.compliance.join(", ").cyan()
                    );
                }
            }
        }
    }

    // Cost
    if report.summary.estimated_monthly_cost != "0 EUR" {
        println!();
        println!(
            "  {} ~{}/month",
            "Estimated cost:".bold(),
            report.summary.estimated_monthly_cost.cyan()
        );
    }

    // Summary line
    println!();
    println!("{}", "-".repeat(60));
    println!(
        "  {} resources across {} providers",
        report.summary.total_resources.to_string().bold(),
        report.summary.providers
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dummy_provider(name: &str) -> Provider {
        Provider {
            name: name.to_string(),
            endpoint: format!("https://{}.example.com", name),
            protocol: "unknown".to_string(),
            capabilities: vec![],
            status: ProviderStatus::Unknown,
            auth_mode: None,
            auth_error: None,
            contract: None,
        }
    }

    #[test]
    fn merge_replaces_capabilities_and_protocol() {
        let manifest = json!({
            "provider": {
                "name": "nuetzliche",
                "description": "MCP-native infra"
            },
            "capabilities": [
                { "type": "scheduling", "product": "croniq", "protocol": "mcp" },
                { "type": "events", "product": "hookaido", "protocol": "mcp" }
            ],
            "region": "eu-central-1",
            "compliance": ["gdpr", "eu-ai-act"]
        });

        let mut p = dummy_provider("legacy-name");
        let info = merge_manifest_into_provider(&manifest, &mut p);

        assert_eq!(p.name, "nuetzliche");
        assert_eq!(p.protocol, "mcp");
        assert_eq!(p.capabilities, vec!["scheduling", "events"]);
        assert_eq!(info.region.as_deref(), Some("eu-central-1"));
        assert_eq!(info.compliance, vec!["gdpr", "eu-ai-act"]);
        assert_eq!(info.description.as_deref(), Some("MCP-native infra"));
    }

    #[test]
    fn merge_keeps_protocol_when_capabilities_disagree() {
        let manifest = json!({
            "provider": { "name": "p" },
            "capabilities": [
                { "type": "a", "protocol": "mcp" },
                { "type": "b", "protocol": "http" }
            ]
        });

        let mut p = dummy_provider("p");
        p.protocol = "unknown".to_string();
        merge_manifest_into_provider(&manifest, &mut p);
        // Two distinct protocols — don't promote either, keep original.
        assert_eq!(p.protocol, "unknown");
        assert_eq!(p.capabilities, vec!["a", "b"]);
    }

    #[test]
    fn merge_noop_on_empty_manifest() {
        let manifest = json!({});
        let mut p = dummy_provider("p");
        p.capabilities = vec!["existing".to_string()];
        p.protocol = "mcp".to_string();
        let info = merge_manifest_into_provider(&manifest, &mut p);

        // No capabilities in manifest → leave existing alone.
        assert_eq!(p.capabilities, vec!["existing"]);
        assert_eq!(p.protocol, "mcp");
        assert!(info.description.is_none());
        assert!(info.region.is_none());
        assert!(info.compliance.is_empty());
    }
}
