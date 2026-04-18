use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    pub detected_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_cost: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub name: String,
    pub endpoint: String,
    pub protocol: String,
    pub capabilities: Vec<String>,
    pub status: ProviderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderStatus {
    Reachable,
    Unreachable,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub workspace: String,
    pub generated: String,
    pub scanner_version: String,
    /// How the scan was run. `online` means provider manifests were fetched;
    /// `offline` means `--no-providers` (or equivalent config) was set.
    /// Optional for backward compatibility with v0.1 reports; consumers
    /// should treat a missing value as `online`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_mode: Option<ScanMode>,
    pub providers: Vec<Provider>,
    pub resources: Vec<Resource>,
    pub summary: Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanMode {
    Online,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub total_resources: usize,
    pub providers: usize,
    pub agent_created: usize,
    pub agent_assisted: usize,
    pub human_created: usize,
    pub auto_detected: usize,
    pub estimated_monthly_cost: String,
}

impl Report {
    pub fn build(
        workspace: String,
        providers: Vec<Provider>,
        resources: Vec<Resource>,
        scan_mode: ScanMode,
    ) -> Self {
        let summary = Summary {
            total_resources: resources.len(),
            providers: providers.len(),
            agent_created: resources
                .iter()
                .filter(|r| {
                    r.created_by
                        .as_ref()
                        .map_or(false, |c| c.starts_with("agent:"))
                })
                .count(),
            agent_assisted: resources
                .iter()
                .filter(|r| {
                    r.extra
                        .get("agent_assisted")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                })
                .count(),
            human_created: resources
                .iter()
                .filter(|r| {
                    r.created_by
                        .as_ref()
                        .map_or(false, |c| c.starts_with("human:"))
                        && !r
                            .extra
                            .get("agent_assisted")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                })
                .count(),
            auto_detected: resources
                .iter()
                .filter(|r| r.created_by.is_none())
                .count(),
            estimated_monthly_cost: aggregate_costs(&resources),
        };

        Report {
            workspace,
            generated: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            scanner_version: env!("CARGO_PKG_VERSION").to_string(),
            scan_mode: Some(scan_mode),
            providers,
            resources,
            summary,
        }
    }
}

fn aggregate_costs(resources: &[Resource]) -> String {
    let mut total_eur: f64 = 0.0;
    for r in resources {
        if let Some(cost) = &r.estimated_cost {
            // Parse "12 EUR/month" or "12.50 EUR/month" style strings
            if let Some(amount) = cost.split_whitespace().next() {
                if let Ok(val) = amount.parse::<f64>() {
                    total_eur += val;
                }
            }
        }
    }
    if total_eur > 0.0 {
        format!("{:.0} EUR", total_eur)
    } else {
        "0 EUR".to_string()
    }
}
