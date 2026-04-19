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
///
/// When `trust == "contract"` an `auth` block is required — composit needs
/// a credential handle to fetch the contract manifest. See RFC 002.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRule {
    pub name: String,
    pub manifest: String,
    pub trust: String,
    #[serde(default)]
    pub compliance: Vec<String>,
    /// Credential handle for contract-tier fetches. `None` when
    /// `trust == "public"`; required when `trust == "contract"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthRef>,
}

/// Credential handle as declared in a Compositfile. composit never reads a
/// secret from the tracked file — `env` names an environment variable that
/// holds the actual credential at scan time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRef {
    /// Method advertised by the provider's public manifest.
    /// Currently `"api-key"`; `"oauth2"` is on the RFC 002 roadmap.
    #[serde(rename = "type")]
    pub auth_type: String,
    /// Name of the environment variable that holds the credential value.
    /// `None` means "no credential configured" — scans fall back to
    /// public-only behaviour and `composit diff` surfaces a
    /// `contract_auth_missing` info diagnostic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
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
