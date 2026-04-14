use serde::{Deserialize, Serialize};

/// The SHOULD-state: governance rules declared in a Compositfile.
/// Compared against the IS-state (composit-report.yaml) by `composit diff`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Governance {
    pub workspace: String,
    pub providers: Vec<ProviderRule>,
    pub budgets: Vec<BudgetRule>,
    pub policies: Vec<PolicyRule>,
    pub resources: Option<ResourceConstraints>,
}

/// An approved provider with manifest URL, trust level, and compliance tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRule {
    pub name: String,
    pub manifest: String,
    pub trust: String,
    #[serde(default)]
    pub compliance: Vec<String>,
}

/// A budget constraint with max monthly cost and optional alert threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetRule {
    pub scope: String,
    pub max_monthly: String,
    pub alert_at: Option<String>,
}

/// A reference to an OPA/Rego policy file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub name: String,
    pub source: String,
    pub description: Option<String>,
}

/// Resource constraints: max totals, allowlists, and required types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConstraints {
    pub max_total: Option<usize>,
    #[serde(default)]
    pub allow: Vec<AllowRule>,
    #[serde(default)]
    pub require: Vec<RequireRule>,
}

/// Whitelist rule for a specific resource type.
/// If at least one AllowRule exists, unlisted types become violations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowRule {
    pub resource_type: String,
    pub max: Option<usize>,
    #[serde(default)]
    pub allowed_images: Vec<String>,
    #[serde(default)]
    pub allowed_types: Vec<String>,
}

/// Require that a resource type exists with at least `min` instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequireRule {
    pub resource_type: String,
    pub min: usize,
}
