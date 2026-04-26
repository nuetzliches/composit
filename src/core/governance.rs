use std::collections::HashMap;

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
    /// Scanner-tuning knobs. Lives inside the Compositfile (as a `scan { … }`
    /// block) so governance and "how to discover" ship together in one
    /// reviewed file — there is no separate composit.config.yaml.
    #[serde(default)]
    pub scan: ScanSettings,
}

/// Tool-level scan configuration — which paths to skip, which custom file
/// patterns to surface, which built-in scanners to disable. Empty defaults
/// everywhere so an omitted `scan { }` block is identical to "no tuning".
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanSettings {
    /// Paths (relative to the scan dir) the filesystem walk must skip.
    /// Bare dir entries expand to `<dir>/**` so `.gitignore`-style habits
    /// work; globs with `*`/`?`/`[` are used verbatim.
    #[serde(default)]
    pub exclude_paths: Vec<String>,

    /// Extra glob patterns that surface as ad-hoc resources with a
    /// user-chosen resource type. Parallels the built-in scanners for
    /// domain-specific files the shipped scanners don't know about.
    #[serde(default)]
    pub extra_patterns: Vec<ExtraPattern>,

    /// Per-scanner on/off override. Missing keys default to enabled.
    #[serde(default)]
    pub scanners: HashMap<String, bool>,

    /// RFC 006: env-file globs whose values may be substituted into other
    /// resources (e.g. `${VAR}` in docker-compose). Values never leave disk.
    /// Keys matching any `redact` pattern are replaced with `<redacted>`.
    ///
    /// `None`       = field absent from Compositfile → diff suggests it when `${VAR}` found.
    /// `Some([])`   = deliberately opted out → diff stays silent.
    /// `Some([..])` = resolution enabled with these env-file globs.
    #[serde(default)]
    pub resolvable: Option<Vec<String>>,

    /// RFC 006: glob-style patterns on env-var *keys* whose values MUST
    /// be redacted even when `resolvable` allows substitution. Matched
    /// case-insensitively against the key name. Defaults apply on top of
    /// anything declared here.
    #[serde(default)]
    pub redact: Vec<String>,

    /// RFC 007: Ansible-specific knobs. Empty defaults mean templates
    /// are discovered but not rendered; set `ansible.extra_vars` to opt
    /// into rendering.
    #[serde(default)]
    pub ansible: AnsibleSettings,
}

/// RFC 007: Ansible rendering knobs.
///
/// v0.1 supported `extra_vars` only. v0.2 adds inventory-aware rendering:
/// each inventory listed in `inventories` produces a separate
/// rendering per template, with variables merged from
/// `all:vars` + matching group:vars + `extra_vars` (highest precedence).
/// `host_vars/` precedence is still deferred — see RFC 007 §Open questions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnsibleSettings {
    /// Variables that override any inventory-provided value. Equivalent
    /// in spirit to `ansible-playbook --extra-vars`. Alone (no
    /// `inventories`), templates render once with just these values.
    #[serde(default)]
    pub extra_vars: HashMap<String, String>,

    /// Paths (relative to scan root) of inventory files to render for.
    /// Each one produces a separate entry in `ansible_template.renderings`.
    /// Omit for extra-vars-only rendering.
    #[serde(default)]
    pub inventories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraPattern {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub glob: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl ScanSettings {
    pub fn is_scanner_enabled(&self, scanner_id: &str) -> bool {
        match self.scanners.get(scanner_id).copied() {
            Some(v) => v,
            // cron reads host-state (`crontab -l`), not repo-state — opt-in only.
            None => scanner_id != "cron",
        }
    }
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
    /// Supported: `"api-key"` (header credential) and `"oauth2"` (client-credentials
    /// grant; `env` holds `client_id:client_secret`).
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
    /// Role sub-blocks — RFC 005. Each role targets a subset of resources of
    /// this type (via matcher predicates) and attaches stricter constraints
    /// than the type-level ones above.
    #[serde(default)]
    pub roles: Vec<Role>,
}

/// Require that a resource type exists with at least `min` instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequireRule {
    pub resource_type: String,
    pub min: usize,
}

/// Role — a named subset of resources within an `allow` block carrying its
/// own constraints. See RFC 005.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    #[serde(default)]
    pub matcher: Matcher,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image_pin: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image_prefix: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub must_expose: Vec<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub must_attach_to: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub must_set_env: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden_env: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub must_have_file: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_count: Option<usize>,
    /// RFC 007 template-rendering constraint: every rendering of a
    /// matched `ansible_template` resource MUST expose the given keys
    /// with values whose glob matches. Applies only when the role
    /// targets `ansible_template` resources (match.type or detection
    /// via resource_type scoping).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub rendered_must_contain: HashMap<String, String>,
}

/// Matcher — selects which resources belong to a role.
/// Empty matcher (all fields empty) selects every resource of the parent type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Matcher {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub name: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<String>,
    #[serde(default)]
    pub predicate: Predicate,
}

impl Matcher {
    pub fn is_empty(&self) -> bool {
        self.name.is_empty() && self.image.is_empty() && self.path.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Predicate {
    #[default]
    All,
    Any,
}
