use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::cli::DiffOutputFormat;
use crate::core::compositfile::parse_compositfile;
use crate::core::governance::{Governance, Predicate, ProviderRule, Role, ScanSettings};
use crate::core::types::{AuthMode, Provider, Report, Resource, ScanMode};

// ─────────────────────────────────────────────────────────
// Violation model
// ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffReport {
    pub workspace: String,
    pub generated: String,
    pub categories: Vec<ViolationCategory>,
    pub summary: DiffSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationCategory {
    pub name: String,
    pub violations: Vec<Violation>,
    pub passed: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Violation {
    pub severity: Severity,
    pub rule: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub details: Option<String>,
    /// What the Compositfile governance requires (SOLL).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub expected: Option<String>,
    /// What the scan actually observed (IST).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub actual: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    #[default]
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub total_violations: usize,
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub passed_checks: usize,
}

// ─────────────────────────────────────────────────────────
// CLI handler
// ─────────────────────────────────────────────────────────

pub fn run_diff(
    dir: &Path,
    compositfile: Option<&Path>,
    report_path: Option<&Path>,
    output: DiffOutputFormat,
    strict: bool,
    offline: bool,
) -> Result<i32> {
    let cf_path = compositfile
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| dir.join("Compositfile"));
    let rp_path = report_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| dir.join("composit-report.yaml"));

    let governance = parse_compositfile(&cf_path)?;

    let report_content = std::fs::read_to_string(&rp_path)
        .with_context(|| format!("Failed to read report: {}", rp_path.display()))?;
    let report: Report = serde_yaml::from_str(&report_content)
        .with_context(|| format!("Failed to parse report YAML: {}", rp_path.display()))?;

    // Offline mode: explicit CLI flag OR the report itself was produced
    // without contacting providers. Both paths collapse to the same
    // behaviour inside compute_diff.
    let effective_offline = offline || matches!(report.scan_mode, Some(ScanMode::Offline));

    let diff = compute_diff_opts(&governance, &report, dir, effective_offline);

    match output {
        DiffOutputFormat::Terminal => print_diff_terminal(&diff),
        DiffOutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&diff)?);
        }
        DiffOutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&diff)?);
        }
        DiffOutputFormat::Html => {
            let html = render_diff_html(&diff);
            let out_path = dir.join("composit-diff.html");
            std::fs::write(&out_path, &html)?;
            println!("  Diff report written to: {}", out_path.display());
        }
    }

    if strict && diff.summary.errors > 0 {
        Ok(1)
    } else {
        Ok(0)
    }
}

// ─────────────────────────────────────────────────────────
// Diff engine
// ─────────────────────────────────────────────────────────

#[cfg(test)]
pub fn compute_diff(governance: &Governance, report: &Report, base_dir: &Path) -> DiffReport {
    compute_diff_opts(governance, report, base_dir, false)
}

/// Same as `compute_diff`, but with a flag that downgrades warnings which
/// only make sense when provider manifests were actually fetched
/// (currently: `unused_provider` → Info).
pub fn compute_diff_opts(
    governance: &Governance,
    report: &Report,
    base_dir: &Path,
    offline: bool,
) -> DiffReport {
    let categories = vec![
        check_workspace(governance, report),
        check_providers(governance, report, offline),
        check_budgets(governance, report),
        check_resources(governance, report),
        check_resolution(report, &governance.scan),
        check_policies(governance, base_dir, report),
    ];

    let errors: usize = categories
        .iter()
        .flat_map(|c| &c.violations)
        .filter(|v| v.severity == Severity::Error)
        .count();
    let warnings: usize = categories
        .iter()
        .flat_map(|c| &c.violations)
        .filter(|v| v.severity == Severity::Warning)
        .count();
    let info: usize = categories
        .iter()
        .flat_map(|c| &c.violations)
        .filter(|v| v.severity == Severity::Info)
        .count();
    let passed: usize = categories.iter().map(|c| c.passed).sum();

    DiffReport {
        workspace: governance.workspace.clone(),
        generated: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        categories,
        summary: DiffSummary {
            total_violations: errors + warnings + info,
            errors,
            warnings,
            info,
            passed_checks: passed,
        },
    }
}

/// Compares the Compositfile `workspace "<name>" { … }` label against the
/// workspace the scanner recorded (the same label, falling back to the
/// directory name when no Compositfile is present). A mismatch means
/// `composit scan` ran somewhere other than where the Compositfile claims,
/// or the Compositfile was renamed but the scan dir wasn't. Surface it as
/// Info so operators notice without failing CI.
fn check_workspace(governance: &Governance, report: &Report) -> ViolationCategory {
    let mut violations = Vec::new();
    let mut passed = 0;

    if governance.workspace == report.workspace {
        passed += 1;
    } else {
        violations.push(Violation {
            severity: Severity::Info,
            rule: "workspace_name_mismatch".to_string(),
            message: format!(
                "Compositfile workspace \"{}\" does not match scan report workspace \"{}\"",
                governance.workspace, report.workspace
            ),
            details: Some(
                "Rename the Compositfile label or the scan directory so they \
                 agree, or run composit scan from the directory whose name \
                 matches the Compositfile workspace."
                    .to_string(),
            ),
            expected: Some(governance.workspace.clone()),
            actual: Some(report.workspace.clone()),
        });
    }

    ViolationCategory {
        name: "workspace".to_string(),
        violations,
        passed,
    }
}

fn check_providers(governance: &Governance, report: &Report, offline: bool) -> ViolationCategory {
    let mut violations = Vec::new();
    let mut passed = 0;

    // Build a name → ProviderRule map so we can correlate report entries
    // back to their governance rule for contract-tier checks (RFC 002).
    let gov_by_name: HashMap<&str, &crate::core::governance::ProviderRule> = governance
        .providers
        .iter()
        .map(|p| (p.name.as_str(), p))
        .collect();

    // Check report providers against approved list
    for rp in &report.providers {
        match gov_by_name.get(rp.name.as_str()) {
            Some(rule) => {
                // Provider is approved. Run RFC 002 contract-tier checks.
                let contract_violations = check_provider_contract(rule, rp, offline);
                if contract_violations.is_empty() {
                    passed += 1;
                } else {
                    violations.extend(contract_violations);
                }
            }
            None => {
                let approved_names: Vec<String> = governance
                    .providers
                    .iter()
                    .map(|p| p.name.clone())
                    .collect();
                violations.push(Violation {
                    severity: Severity::Error,
                    rule: "unapproved_provider".to_string(),
                    message: format!(
                        "Provider \"{}\" found in report but not approved in Compositfile",
                        rp.name
                    ),
                    details: Some(format!("Endpoint: {}", rp.endpoint)),
                    expected: Some(if approved_names.is_empty() {
                        "(no approved providers)".to_string()
                    } else {
                        approved_names.join("\n")
                    }),
                    actual: Some(rp.name.clone()),
                });
            }
        }
    }

    // Check approved providers not in report.
    // Offline scans don't attempt manifest discovery, so the report
    // lacks remote providers by construction — a mismatch here is
    // expected, not a red flag. Downgrade to Info with an explanatory
    // details field so the signal still shows up but doesn't block.
    let report_names: Vec<&str> = report.providers.iter().map(|p| p.name.as_str()).collect();
    for gp in &governance.providers {
        if report_names.contains(&gp.name.as_str()) {
            continue;
        }
        let (severity, details) = if offline {
            (
                Severity::Info,
                Some(
                    "scan ran offline (--no-providers); run without --no-providers to verify"
                        .to_string(),
                ),
            )
        } else {
            (Severity::Warning, None)
        };
        violations.push(Violation {
            severity,
            rule: "unused_provider".to_string(),
            message: format!(
                "Approved provider \"{}\" not found in scan report — governance may be outdated",
                gp.name
            ),
            details,
            expected: Some(gp.name.clone()),
            actual: Some("(not observed in scan)".to_string()),
        });
    }

    ViolationCategory {
        name: "providers".to_string(),
        violations,
        passed,
    }
}

/// Run the RFC 002 contract-tier checks for a single approved provider.
/// Returns zero violations when everything lines up — the caller treats
/// that as a passing check.
///
/// Rule coverage (RFC 002 §scan/diff behaviour):
/// - `contract_auth_missing`     Info    — trust=contract, no credential configured
/// - `contract_auth_mismatch`    Warning — gov auth.type != public manifest's advertised type
/// - `contract_unreachable`      Warning — public manifest itself was unreachable
/// - `contract_unauthorized`     Error   — credential present, contract URL returned 401/403
/// - `contract_expired`          Error   — contract manifest reports expired
///
/// `contract_auth_mismatch` is detected at scan time (recorded as
/// `auth_error = "auth_type_not_advertised"`). `contract_expired` fires
/// when the RFC 003 `contract.expires_at` timestamp is in the past
/// against the scanner's local UTC clock.
fn check_provider_contract(
    rule: &ProviderRule,
    report_provider: &Provider,
    offline: bool,
) -> Vec<Violation> {
    let mut violations = Vec::new();

    // Unreachable: flag regardless of trust level — governance expected
    // the provider to exist, the scanner couldn't reach it.
    if matches!(report_provider.auth_mode, Some(AuthMode::Unreachable)) {
        violations.push(Violation {
            severity: Severity::Warning,
            rule: "contract_unreachable".to_string(),
            message: format!("Provider \"{}\" was unreachable during the scan", rule.name),
            details: Some(format!("Endpoint: {}", report_provider.endpoint)),
            ..Default::default()
        });
        return violations;
    }

    // Contract-tier checks apply only when the governance rule asked for it.
    if rule.trust != "contract" {
        return violations;
    }

    // auth_mode defaults to Public when missing (back-compat with reports
    // produced before the field existed).
    let observed = report_provider.auth_mode.unwrap_or(AuthMode::Public);

    match observed {
        AuthMode::Contract => {
            // Upgrade succeeded. Only outstanding verdict is expiry —
            // the scanner records contract.expires_at from the RFC 003
            // response; compare it against the local clock.
            if let Some(info) = report_provider.contract.as_ref() {
                match DateTime::parse_from_rfc3339(&info.expires_at) {
                    Ok(ts) => {
                        let expires_utc = ts.with_timezone(&Utc);
                        let now = Utc::now();
                        if expires_utc < now {
                            let days_past = (now - expires_utc).num_days();
                            violations.push(Violation {
                                severity: Severity::Error,
                                rule: "contract_expired".to_string(),
                                message: format!(
                                    "Provider \"{}\" contract expired at {} ({} day{} ago). \
                                     Renew or rotate the governance entry.",
                                    rule.name,
                                    info.expires_at,
                                    days_past,
                                    if days_past == 1 { "" } else { "s" },
                                ),
                                details: info
                                    .pricing_tier
                                    .as_ref()
                                    .map(|t| format!("pricing tier: {}", t)),
                                ..Default::default()
                            });
                        }
                    }
                    Err(_) => {
                        // Scanner accepted the body but the timestamp is
                        // malformed. Treat as a body-shape problem, not an
                        // expiry verdict — reported as contract_unreachable
                        // so an operator sees it without it masquerading
                        // as a policy failure.
                        violations.push(Violation {
                            severity: Severity::Warning,
                            rule: "contract_unreachable".to_string(),
                            message: format!(
                                "Provider \"{}\": contract.expires_at is not a valid ISO-8601 timestamp",
                                rule.name
                            ),
                            details: Some(format!("received: {:?}", info.expires_at)),
                            ..Default::default()
                        });
                    }
                }
            }
        }
        AuthMode::Public => {
            // Scanner either didn't attempt the upgrade or it failed.
            // The specific reason is in auth_error (set by the scanner).
            let reason = report_provider.auth_error.as_deref().unwrap_or("");
            match reason {
                "unauthorized" => {
                    violations.push(Violation {
                        severity: Severity::Error,
                        rule: "contract_unauthorized".to_string(),
                        message: format!(
                            "Provider \"{}\" rejected the configured credential (401/403). \
                             Contract is stale, revoked, or the wrong key was configured.",
                            rule.name
                        ),
                        details: rule
                            .auth
                            .as_ref()
                            .and_then(|a| a.env.as_ref().map(|e| format!("credential env: {}", e))),
                        ..Default::default()
                    });
                }
                "auth_type_not_advertised" => {
                    violations.push(Violation {
                        severity: Severity::Warning,
                        rule: "contract_auth_mismatch".to_string(),
                        message: format!(
                            "Provider \"{}\": Compositfile declares auth.type = {:?} but the public \
                             manifest does not advertise a contract endpoint with that type. \
                             Update one side to match.",
                            rule.name,
                            rule.auth.as_ref().map(|a| a.auth_type.as_str()).unwrap_or("api-key"),
                        ),
                        details: None,
                        ..Default::default()
                    });
                }
                "fetch_failed" => {
                    violations.push(Violation {
                        severity: Severity::Warning,
                        rule: "contract_unreachable".to_string(),
                        message: format!(
                            "Provider \"{}\": contract fetch failed (network error, 5xx, or invalid body)",
                            rule.name
                        ),
                        details: None,
                        ..Default::default()
                    });
                }
                "invalid_contract_body" => {
                    violations.push(Violation {
                        severity: Severity::Warning,
                        rule: "contract_unreachable".to_string(),
                        message: format!(
                            "Provider \"{}\": contract response did not match the RFC 003 v0.1 envelope",
                            rule.name
                        ),
                        details: Some(
                            "missing or malformed contract.{id, provider, issued_at, expires_at}, or contract.provider did not match the public manifest"
                                .to_string(),
                        ),
                        ..Default::default()
                    });
                }
                // Empty reason OR "auth_missing": the scanner didn't
                // have a credential to try. Info, not warning — CI that
                // runs offline legitimately lands here.
                _ => {
                    let (severity, details) = if offline {
                        (
                            Severity::Info,
                            Some("scan ran offline; credential not checked".to_string()),
                        )
                    } else {
                        (
                            Severity::Info,
                            Some(
                                "set the env var named in auth.env on the Compositfile to verify the contract tier"
                                    .to_string(),
                            ),
                        )
                    };
                    violations.push(Violation {
                        severity,
                        rule: "contract_auth_missing".to_string(),
                        message: format!(
                            "Provider \"{}\" declared trust = \"contract\" but the scan \
                             could not attempt the upgrade (no credential available).",
                            rule.name
                        ),
                        details,
                        ..Default::default()
                    });
                }
            }
        }
        AuthMode::Unreachable => {
            // Covered above; unreachable.
            unreachable!("unreachable auth_mode already handled");
        }
    }

    violations
}

fn check_budgets(governance: &Governance, report: &Report) -> ViolationCategory {
    let mut violations = Vec::new();
    let mut passed = 0;

    let report_cost = parse_cost(&report.summary.estimated_monthly_cost);

    for budget in &governance.budgets {
        if budget.scope != "workspace" {
            continue; // Only workspace budget is checkable against report summary
        }

        if let (Some(actual), Some(max)) = (report_cost, parse_cost(&budget.max_monthly)) {
            if actual > max {
                violations.push(Violation {
                    severity: Severity::Error,
                    rule: "budget_exceeded".to_string(),
                    message: format!(
                        "Workspace cost {} exceeds budget {}",
                        report.summary.estimated_monthly_cost, budget.max_monthly
                    ),
                    details: None,
                    expected: Some(format!("≤ {}", budget.max_monthly)),
                    actual: Some(report.summary.estimated_monthly_cost.clone()),
                });
            } else if let Some(alert_str) = &budget.alert_at {
                if let Some(threshold) = parse_percentage(alert_str) {
                    let alert_amount = max * threshold;
                    if actual > alert_amount {
                        violations.push(Violation {
                            severity: Severity::Warning,
                            rule: "budget_alert".to_string(),
                            message: format!(
                                "Workspace cost {} exceeds {}% alert threshold ({:.0} EUR)",
                                report.summary.estimated_monthly_cost,
                                (threshold * 100.0) as usize,
                                alert_amount
                            ),
                            details: None,
                            expected: Some(format!(
                                "≤ {:.0} EUR ({}% of {})",
                                alert_amount,
                                (threshold * 100.0) as usize,
                                budget.max_monthly
                            )),
                            actual: Some(report.summary.estimated_monthly_cost.clone()),
                        });
                    } else {
                        passed += 1;
                    }
                } else {
                    passed += 1;
                }
            } else {
                passed += 1;
            }
        }
    }

    ViolationCategory {
        name: "budgets".to_string(),
        violations,
        passed,
    }
}

fn check_resources(governance: &Governance, report: &Report) -> ViolationCategory {
    let mut violations = Vec::new();
    let mut passed = 0;

    let constraints = match &governance.resources {
        Some(c) => c,
        None => {
            return ViolationCategory {
                name: "resources".to_string(),
                violations,
                passed: 1, // No constraints = pass
            };
        }
    };

    // Check max_total
    if let Some(max_total) = constraints.max_total {
        if report.summary.total_resources > max_total {
            violations.push(Violation {
                severity: Severity::Error,
                rule: "resource_count_exceeded".to_string(),
                message: format!(
                    "Total resources {} exceeds max_total {}",
                    report.summary.total_resources, max_total
                ),
                details: None,
                ..Default::default()
            });
        } else {
            passed += 1;
        }
    }

    // Group report resources by type
    let mut by_type: HashMap<&str, Vec<&crate::core::types::Resource>> = HashMap::new();
    for r in &report.resources {
        by_type.entry(&r.resource_type).or_default().push(r);
    }

    // Allowlist mode: if at least one allow rule exists, unlisted types are violations
    let allowlist_mode = !constraints.allow.is_empty();
    let allowed_types: Vec<&str> = constraints
        .allow
        .iter()
        .map(|a| a.resource_type.as_str())
        .collect();

    if allowlist_mode {
        for resource_type in by_type.keys() {
            if !allowed_types.contains(resource_type) {
                let count = by_type[resource_type].len();
                violations.push(Violation {
                    severity: Severity::Error,
                    rule: "resource_type_not_allowed".to_string(),
                    message: format!(
                        "Resource type \"{}\" ({} found) not in allow list",
                        resource_type, count
                    ),
                    details: None,
                    expected: Some("(not in allow list)".to_string()),
                    actual: Some(format!("{} × {}", count, resource_type)),
                });
            }
        }
    }

    // Check allow rules: max counts
    for rule in &constraints.allow {
        let count = by_type
            .get(rule.resource_type.as_str())
            .map(|v| v.len())
            .unwrap_or(0);

        if let Some(max) = rule.max {
            if count > max {
                violations.push(Violation {
                    severity: Severity::Error,
                    rule: "resource_type_max_exceeded".to_string(),
                    message: format!(
                        "{}: {} found, max allowed is {}",
                        rule.resource_type, count, max
                    ),
                    details: None,
                    expected: Some(format!("≤ {}", max)),
                    actual: Some(count.to_string()),
                });
            } else {
                passed += 1;
            }
        }

        // Check allowed_images for docker_service
        if !rule.allowed_images.is_empty() {
            if let Some(resources) = by_type.get(rule.resource_type.as_str()) {
                for r in resources {
                    if let Some(image) = r.extra.get("image").and_then(|v| v.as_str()) {
                        if !matches_any_pattern(image, &rule.allowed_images) {
                            violations.push(Violation {
                                severity: Severity::Error,
                                rule: "image_not_allowed".to_string(),
                                message: format!(
                                    "Image \"{}\" not in allowed list for {}{}",
                                    image,
                                    rule.resource_type,
                                    provenance_suffix(r)
                                ),
                                details: r.path.clone(),
                                expected: Some(rule.allowed_images.join("\n")),
                                actual: Some(image.to_string()),
                            });
                        }
                    }
                }
            }
        }

        // Check allowed_types for terraform_resource
        if !rule.allowed_types.is_empty() {
            if let Some(resources) = by_type.get(rule.resource_type.as_str()) {
                for r in resources {
                    if let Some(rt) = r.extra.get("resource_type").and_then(|v| v.as_str()) {
                        if !matches_any_pattern(rt, &rule.allowed_types) {
                            violations.push(Violation {
                                severity: Severity::Error,
                                rule: "resource_subtype_not_allowed".to_string(),
                                message: format!(
                                    "Resource type \"{}\" not in allowed types for {}{}",
                                    rt,
                                    rule.resource_type,
                                    provenance_suffix(r)
                                ),
                                details: r.path.clone(),
                                expected: Some(rule.allowed_types.join("\n")),
                                actual: Some(rt.to_string()),
                            });
                        }
                    }
                }
            }
        }

        // RFC 005: per-role constraints inside this allow block.
        if !rule.roles.is_empty() {
            let empty: Vec<&Resource> = Vec::new();
            let resources_of_type: &[&Resource] = by_type
                .get(rule.resource_type.as_str())
                .map(|v| v.as_slice())
                .unwrap_or(&empty);
            for role in &rule.roles {
                let matched: Vec<&Resource> = resources_of_type
                    .iter()
                    .copied()
                    .filter(|r| role_matches(role, r))
                    .collect();
                check_role_constraints(
                    role,
                    &rule.resource_type,
                    &matched,
                    &mut violations,
                    &mut passed,
                );
            }
        }
    }

    // Check require rules
    for rule in &constraints.require {
        let count = by_type
            .get(rule.resource_type.as_str())
            .map(|v| v.len())
            .unwrap_or(0);

        if count < rule.min {
            violations.push(Violation {
                severity: Severity::Error,
                rule: "required_resource_missing".to_string(),
                message: format!(
                    "Required resource type \"{}\": {} found, minimum is {}",
                    rule.resource_type, count, rule.min
                ),
                details: None,
                expected: Some(format!("≥ {}", rule.min)),
                actual: Some(count.to_string()),
            });
        } else {
            passed += 1;
        }
    }

    ViolationCategory {
        name: "resources".to_string(),
        violations,
        passed,
    }
}

/// Surface RFC 006 resolution artefacts as diff signals. Emits:
/// - `resolution_disabled` (Info) once, when the report carries no resolution
///   metadata, `${VAR}` references were found, AND the Compositfile has no
///   `scan { resolvable = [...] }` block (i.e. `scan.resolvable` is `None`).
///   Silenced when `resolvable = []` is declared explicitly — that signals a
///   deliberate opt-out.
/// - `unresolved_variable` (Info) per variable that couldn't be filled in by
///   the resolver.
fn check_resolution(report: &Report, scan: &ScanSettings) -> ViolationCategory {
    let mut violations = Vec::new();
    let mut passed = 0;

    let has_templated = report.resources.iter().any(|r| {
        r.extra
            .get("image")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains("${"))
            || r.extra
                .get("ports")
                .and_then(|v| v.as_array())
                .is_some_and(|arr| {
                    arr.iter()
                        .any(|p| p.as_str().is_some_and(|s| s.contains("${")))
                })
    });

    match &report.resolution {
        None => {
            // Only nag when resolvable is absent entirely — `resolvable = []`
            // means the operator deliberately opted out and wants silence.
            if has_templated && scan.resolvable.is_none() {
                violations.push(Violation {
                    severity: Severity::Info,
                    rule: "resolution_disabled".to_string(),
                    message: "Scan found ${VAR} references but cross-file resolution is disabled"
                        .to_string(),
                    details: Some(
                        "`composit init` writes a commented `scan { resolvable = [\".env\"] }` \
                         block when env files are detected — uncomment it to enable RFC 006 \
                         variable substitution. If your Compositfile is hand-written, add the \
                         block at the top of the workspace block. Default redaction applies to \
                         *_KEY, *_SECRET, *_TOKEN, *_PASSWORD. \
                         To silence this diagnostic permanently, set `scan { resolvable = [] }`."
                            .to_string(),
                    ),
                    expected: Some("resolvable = [\".env\"]".to_string()),
                    actual: Some("(not set)".to_string()),
                });
            } else {
                passed += 1;
            }
        }
        Some(res) => {
            if res.unresolved.is_empty() {
                passed += 1;
            }
            for u in &res.unresolved {
                violations.push(Violation {
                    severity: Severity::Info,
                    rule: "unresolved_variable".to_string(),
                    message: format!(
                        "\"${{{}}}\" referenced in {} of {} but not defined in any resolvable env file",
                        u.variable, u.field, u.resource_path
                    ),
                    details: Some(format!(
                        "env_files_used: {}",
                        if res.env_files_used.is_empty() {
                            "(none)".to_string()
                        } else {
                            res.env_files_used.join(", ")
                        }
                    )),
                    expected: Some(u.variable.clone()),
                    actual: Some("(undefined)".to_string()),
                });
            }
        }
    }

    // RFC 007 §Open question 1: vault-encrypted templates are visible
    // in the scan but never rendered. Surface them as Info so operators
    // see that their governance doesn't see inside those files.
    for r in &report.resources {
        if r.resource_type == "ansible_template"
            && r.extra
                .get("vault_encrypted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            violations.push(Violation {
                severity: Severity::Info,
                rule: "vault_unsupported".to_string(),
                message: format!(
                    "Template {} is ansible-vault encrypted and was not rendered{}",
                    r.path.as_deref().unwrap_or("-"),
                    provenance_suffix(r)
                ),
                details: Some(
                    "composit does not decrypt vault files. Role constraints on \
                     the rendered output cannot apply until decryption is wired in."
                        .to_string(),
                ),
                expected: Some("plaintext template".to_string()),
                actual: Some("$ANSIBLE_VAULT;...".to_string()),
            });
        }
    }

    ViolationCategory {
        name: "resolution".to_string(),
        violations,
        passed,
    }
}

fn check_policies(governance: &Governance, base_dir: &Path, report: &Report) -> ViolationCategory {
    use crate::core::opa_eval::{eval_policy, PolicyOutcome};
    use crate::core::rego::{parse_rego, RegoIssue};

    // Serialise the scan report once; every policy evaluates against the
    // same input so we only pay the serialisation cost once.
    let input_json = serde_json::to_string(report).unwrap_or_else(|_| "{}".to_string());

    let mut violations = Vec::new();
    let mut passed = 0;

    for policy in &governance.policies {
        let policy_path = base_dir.join(&policy.source);

        // Only .rego files get parsed. Other source types (a .md file,
        // a link to an external system) are checked for existence only.
        let is_rego = policy_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("rego"));

        if !policy_path.exists() {
            violations.push(Violation {
                severity: Severity::Warning,
                rule: "policy_file_missing".to_string(),
                message: format!(
                    "Policy \"{}\" references missing file: {}",
                    policy.name, policy.source
                ),
                details: None,
                ..Default::default()
            });
            continue;
        }

        if !is_rego {
            // Non-Rego reference (docs, external spec) — existence check
            // is all we can do today.
            passed += 1;
            continue;
        }

        let content = match std::fs::read_to_string(&policy_path) {
            Ok(c) => c,
            Err(e) => {
                violations.push(Violation {
                    severity: Severity::Warning,
                    rule: "policy_file_unreadable".to_string(),
                    message: format!("Policy \"{}\" could not be read: {}", policy.name, e),
                    details: Some(policy.source.clone()),
                    ..Default::default()
                });
                continue;
            }
        };

        let meta = match parse_rego(&content) {
            Ok(m) => m,
            Err(issue) => {
                let rule = match issue {
                    RegoIssue::MissingPackage => "policy_missing_package",
                    RegoIssue::UnbalancedBraces { .. } => "policy_syntax_error",
                    RegoIssue::Empty => "policy_empty",
                };
                violations.push(Violation {
                    severity: Severity::Warning,
                    rule: rule.to_string(),
                    message: format!(
                        "Policy \"{}\" ({}) could not be parsed: {}",
                        policy.name, policy.source, issue
                    ),
                    details: None,
                    ..Default::default()
                });
                continue;
            }
        };

        // ── Runtime evaluation ───────────────────────────────────────────
        // Policies that declare deny or allow entrypoints are evaluated
        // against the full scan report. Policies with neither entrypoint
        // are informational (e.g. documentation stubs) and skip eval.
        let can_eval = meta.has_deny || meta.has_default_allow;
        if can_eval {
            let outcome = eval_policy(
                &policy.source,
                &content,
                &meta.package,
                meta.has_deny,
                meta.has_default_allow,
                &input_json,
            );

            match outcome {
                PolicyOutcome::Clean => {
                    passed += 1;
                    violations.push(Violation {
                        severity: Severity::Info,
                        rule: "policy_passed".to_string(),
                        message: format!(
                            "Policy \"{}\" ({}): all checks passed",
                            policy.name, policy.source
                        ),
                        details: None,
                        ..Default::default()
                    });
                }
                PolicyOutcome::Denials(msgs) => {
                    // Each deny message becomes a separate Error so CI gates
                    // on individual violations rather than a single policy blob.
                    for msg in msgs {
                        violations.push(Violation {
                            severity: Severity::Error,
                            rule: "policy_violation".to_string(),
                            message: format!(
                                "Policy \"{}\" ({}): {}",
                                policy.name, policy.source, msg
                            ),
                            details: None,
                            ..Default::default()
                        });
                    }
                }
                PolicyOutcome::NotAllowed => {
                    violations.push(Violation {
                        severity: Severity::Error,
                        rule: "policy_not_allowed".to_string(),
                        message: format!(
                            "Policy \"{}\" ({}): allow evaluated to false",
                            policy.name, policy.source
                        ),
                        details: None,
                        ..Default::default()
                    });
                }
                PolicyOutcome::EvalError(e) => {
                    // Evaluation errors (unsupported features, runtime panics)
                    // are surfaced as warnings rather than errors so a single
                    // Rego feature gap doesn't fail CI.
                    violations.push(Violation {
                        severity: Severity::Warning,
                        rule: "policy_eval_error".to_string(),
                        message: format!(
                            "Policy \"{}\" ({}) could not be evaluated: {}",
                            policy.name, policy.source, e
                        ),
                        details: None,
                        ..Default::default()
                    });
                }
            }
        } else {
            // No executable entrypoint — keep the legacy policy_parsed info
            // so the diff report still acknowledges the file exists.
            passed += 1;
            violations.push(Violation {
                severity: Severity::Info,
                rule: "policy_parsed".to_string(),
                message: format!(
                    "Policy \"{}\" ({}): package `{}`, {} rule(s) — no deny/allow entrypoint",
                    policy.name,
                    policy.source,
                    meta.package,
                    meta.rules.len(),
                ),
                details: if meta.rules.is_empty() {
                    None
                } else {
                    Some(format!("rules: {}", meta.rules.join(", ")))
                },
                ..Default::default()
            });
        }
    }

    ViolationCategory {
        name: "policies".to_string(),
        violations,
        passed,
    }
}

// ─────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────

fn parse_cost(s: &str) -> Option<f64> {
    s.split_whitespace().next()?.parse::<f64>().ok()
}

/// Parse a percentage string like "80%" into a [0.0, 1.0] fraction.
/// Rejects values outside 0-100%.
fn parse_percentage(s: &str) -> Option<f64> {
    let v = s.trim_end_matches('%').parse::<f64>().ok()?;
    if !(0.0..=100.0).contains(&v) {
        return None;
    }
    Some(v / 100.0)
}

fn matches_any_pattern(value: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|p| glob::Pattern::new(p).is_ok_and(|pat| pat.matches(value)))
}

// ─────────────────────────────────────────────────────────
// RFC 005 — role matching & constraint checks
// ─────────────────────────────────────────────────────────

/// Does a resource belong to this role? Applies the role's matcher.
/// An empty matcher (no attributes set) selects every resource.
fn role_matches(role: &Role, r: &Resource) -> bool {
    let m = &role.matcher;
    if m.is_empty() {
        return true;
    }

    // Per matcher attribute: matches if the attribute is empty OR the resource
    // satisfies at least one pattern inside that attribute.
    let name_ok = m.name.is_empty()
        || r.name
            .as_deref()
            .is_some_and(|n| matches_any_pattern(n, &m.name));
    let image_ok = m.image.is_empty()
        || image_for_matching(r).is_some_and(|i| matches_any_pattern(&i, &m.image));
    let path_ok = m.path.is_empty()
        || r.path
            .as_deref()
            .map(normalize_path)
            .is_some_and(|p| matches_any_pattern(&p, &m.path));

    let attrs_set = [!m.name.is_empty(), !m.image.is_empty(), !m.path.is_empty()];
    let results = [name_ok, image_ok, path_ok];

    match m.predicate {
        Predicate::All => attrs_set
            .iter()
            .zip(results.iter())
            .all(|(set, ok)| !*set || *ok),
        Predicate::Any => attrs_set
            .iter()
            .zip(results.iter())
            .any(|(set, ok)| *set && *ok),
    }
}

/// Prefer the RFC 006 `resolved_image` when present — role constraints
/// care about the concrete image that will run, not the `${VAR}` template.
fn image_for_matching(r: &Resource) -> Option<String> {
    if let Some(s) = r.extra.get("resolved_image").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    r.extra
        .get("image")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// Normalize a resource path for glob matching: strip leading `./` and
/// convert Windows backslashes so patterns written with `/` work cross-platform.
fn normalize_path(p: &str) -> String {
    let trimmed = p.strip_prefix("./").unwrap_or(p);
    trimmed.replace('\\', "/")
}

fn string_list(items: &[String]) -> String {
    items.join("\n")
}

/// Issue #20: when a resource carries a `provenance` block (set by the
/// post-scan `apply_provenance` pass), append `(source: <kind> <ref>)` to
/// the violation message so reports point operators to the upstream spec
/// rather than the generated artefact. Empty string when no provenance
/// is present, so callers can append unconditionally.
fn provenance_suffix(r: &Resource) -> String {
    let Some(prov) = r.extra.get("provenance").and_then(|v| v.as_object()) else {
        return String::new();
    };
    let kind = prov.get("source_kind").and_then(|v| v.as_str());
    let source_ref = prov.get("source_ref").and_then(|v| v.as_str());
    match (kind, source_ref) {
        (Some(k), Some(rf)) => format!(" (source: {} {})", k, rf),
        (Some(k), None) => format!(" (source: {})", k),
        (None, Some(rf)) => format!(" (source: {})", rf),
        (None, None) => String::new(),
    }
}

/// Human-readable summary of which resources a role matched. Used in the
/// `details` field of role violations so the HTML diff shows authors
/// *which* services tripped the rule, not just how many.
fn matched_summary(matched: &[&Resource]) -> String {
    if matched.is_empty() {
        return "(no matches)".to_string();
    }
    let names: Vec<String> = matched
        .iter()
        .take(5)
        .map(|r| match (r.name.as_deref(), r.path.as_deref()) {
            (Some(n), Some(p)) => format!("{} ({})", n, p),
            (Some(n), None) => n.to_string(),
            (None, Some(p)) => p.to_string(),
            (None, None) => "-".to_string(),
        })
        .collect();
    let mut s = names.join("; ");
    if matched.len() > 5 {
        s.push_str(&format!(" … (+{} more)", matched.len() - 5));
    }
    s
}

/// Evaluate all constraints of a role against the set of resources that
/// matched it. Emits per-resource violations (e.g. image_not_pinned) and
/// per-role violations (min/max_count) with `expected`/`actual` populated
/// for the HTML diff renderer.
fn check_role_constraints(
    role: &Role,
    resource_type: &str,
    matched: &[&Resource],
    violations: &mut Vec<Violation>,
    passed: &mut usize,
) {
    let role_tag = format!("role: \"{}\"", role.name);

    // Count-based constraints evaluated once per role.
    if let Some(min) = role.min_count {
        if matched.len() < min {
            violations.push(Violation {
                severity: Severity::Error,
                rule: "role_count_below_min".to_string(),
                message: format!(
                    "Role \"{}\" on {}: {} matching, minimum is {}",
                    role.name,
                    resource_type,
                    matched.len(),
                    min
                ),
                details: Some(format!(
                    "{} — matched: {}",
                    role_tag,
                    matched_summary(matched)
                )),
                expected: Some(format!("≥ {}", min)),
                actual: Some(matched.len().to_string()),
            });
        } else {
            *passed += 1;
        }
    }

    if let Some(max) = role.max_count {
        if matched.len() > max {
            violations.push(Violation {
                severity: Severity::Error,
                rule: "role_count_above_max".to_string(),
                message: format!(
                    "Role \"{}\" on {}: {} matching, maximum is {}",
                    role.name,
                    resource_type,
                    matched.len(),
                    max
                ),
                details: Some(format!(
                    "{} — matched: {}",
                    role_tag,
                    matched_summary(matched)
                )),
                expected: Some(format!("≤ {}", max)),
                actual: Some(matched.len().to_string()),
            });
        } else {
            *passed += 1;
        }
    }

    // Per-resource constraints.
    for r in matched {
        let path = r.path.as_deref().unwrap_or("-");
        let detail = format!("{} @ {}", role_tag, path);

        // image_pin — exact (glob) match against a list of allowed pins.
        // Uses the RFC-006 resolved form when present so `${VAR:-latest}`
        // → `postgres:16` is compared against the pin, not the template.
        if !role.image_pin.is_empty() {
            if let Some(image) = image_for_matching(r) {
                if !matches_any_pattern(&image, &role.image_pin) {
                    violations.push(Violation {
                        severity: Severity::Error,
                        rule: "role_image_not_pinned".to_string(),
                        message: format!(
                            "Role \"{}\": image \"{}\" not in pinned list{}",
                            role.name,
                            image,
                            provenance_suffix(r)
                        ),
                        details: Some(detail.clone()),
                        expected: Some(string_list(&role.image_pin)),
                        actual: Some(image),
                    });
                } else {
                    *passed += 1;
                }
            }
        }

        // image_prefix — the image must start with one of the listed strings.
        if !role.image_prefix.is_empty() {
            if let Some(image) = image_for_matching(r) {
                let ok = role.image_prefix.iter().any(|pfx| image.starts_with(pfx));
                if !ok {
                    violations.push(Violation {
                        severity: Severity::Error,
                        rule: "role_image_prefix_mismatch".to_string(),
                        message: format!(
                            "Role \"{}\": image \"{}\" does not match any allowed prefix{}",
                            role.name,
                            image,
                            provenance_suffix(r)
                        ),
                        details: Some(detail.clone()),
                        expected: Some(string_list(&role.image_prefix)),
                        actual: Some(image),
                    });
                } else {
                    *passed += 1;
                }
            }
        }

        // must_expose — each required port must appear in the resource's ports list.
        if !role.must_expose.is_empty() {
            let observed_ports = extract_container_ports(r);
            let missing: Vec<u16> = role
                .must_expose
                .iter()
                .copied()
                .filter(|p| !observed_ports.contains(p))
                .collect();
            if !missing.is_empty() {
                violations.push(Violation {
                    severity: Severity::Error,
                    rule: "role_port_missing".to_string(),
                    message: format!(
                        "Role \"{}\": required ports {:?} not exposed (missing {:?}){}",
                        role.name,
                        role.must_expose,
                        missing,
                        provenance_suffix(r)
                    ),
                    details: Some(detail.clone()),
                    expected: Some(
                        role.must_expose
                            .iter()
                            .map(|p| p.to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                    actual: Some(if observed_ports.is_empty() {
                        "(none)".to_string()
                    } else {
                        observed_ports
                            .iter()
                            .map(|p| p.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    }),
                });
            } else {
                *passed += 1;
            }
        }

        // must_attach_to — required networks.
        if !role.must_attach_to.is_empty() {
            let networks = extract_networks(r);
            let missing: Vec<&String> = role
                .must_attach_to
                .iter()
                .filter(|n| !networks.iter().any(|actual| actual == *n))
                .collect();
            if !missing.is_empty() {
                violations.push(Violation {
                    severity: Severity::Error,
                    rule: "role_network_missing".to_string(),
                    message: format!(
                        "Role \"{}\": not attached to required networks ({}){}",
                        role.name,
                        missing
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                        provenance_suffix(r)
                    ),
                    details: Some(detail.clone()),
                    expected: Some(string_list(&role.must_attach_to)),
                    actual: Some(if networks.is_empty() {
                        "(none)".to_string()
                    } else {
                        networks.join(", ")
                    }),
                });
            } else {
                *passed += 1;
            }
        }

        // must_set_env / forbidden_env — apply to env_file resources.
        if !role.must_set_env.is_empty() || !role.forbidden_env.is_empty() {
            let observed_env = extract_env_keys(r);
            if !role.must_set_env.is_empty() {
                let missing: Vec<&String> = role
                    .must_set_env
                    .iter()
                    .filter(|name| !observed_env.iter().any(|k| k == *name))
                    .collect();
                if !missing.is_empty() {
                    violations.push(Violation {
                        severity: Severity::Error,
                        rule: "role_env_var_missing".to_string(),
                        message: format!(
                            "Role \"{}\": env vars not set: {}{}",
                            role.name,
                            missing
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(", "),
                            provenance_suffix(r)
                        ),
                        details: Some(detail.clone()),
                        expected: Some(string_list(&role.must_set_env)),
                        actual: Some(if observed_env.is_empty() {
                            "(none)".to_string()
                        } else {
                            observed_env.join(", ")
                        }),
                    });
                } else {
                    *passed += 1;
                }
            }
            if !role.forbidden_env.is_empty() {
                let present: Vec<&String> = role
                    .forbidden_env
                    .iter()
                    .filter(|name| observed_env.iter().any(|k| k == *name))
                    .collect();
                if !present.is_empty() {
                    violations.push(Violation {
                        severity: Severity::Error,
                        rule: "role_env_var_forbidden".to_string(),
                        message: format!(
                            "Role \"{}\": forbidden env vars present: {}{}",
                            role.name,
                            present
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(", "),
                            provenance_suffix(r)
                        ),
                        details: Some(detail.clone()),
                        expected: Some(string_list(&role.forbidden_env)),
                        actual: Some(
                            present
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(", "),
                        ),
                    });
                } else {
                    *passed += 1;
                }
            }
        }
    }

    // rendered_must_contain — per-resource: for every matched
    // ansible_template, every rendering must expose the named keys
    // whose values match the declared glob pattern. Parsed-dotenv keys
    // are checked first; a substring match on the raw rendering is the
    // fallback when no structured form was recognised.
    if !role.rendered_must_contain.is_empty() {
        for r in matched {
            if r.resource_type != "ansible_template" {
                continue;
            }
            let path = r.path.as_deref().unwrap_or("-");
            let renderings = r
                .extra
                .get("renderings")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for rendering in &renderings {
                let src_tag = rendering
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                for (key, expected_glob) in &role.rendered_must_contain {
                    let actual_val = rendering
                        .get("rendered_parsed")
                        .and_then(|p| p.get("keys"))
                        .and_then(|k| k.get(key))
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    let ok = match actual_val.as_deref() {
                        Some(v) => glob::Pattern::new(expected_glob)
                            .map(|p| p.matches(v))
                            .unwrap_or(false),
                        None => {
                            // Fallback for unparsed formats: look for
                            // "<key> <expected>" substring in the raw
                            // render. Conservative (false negatives
                            // possible) but avoids false positives.
                            let rendered = rendering
                                .get("rendered")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            rendered.contains(key)
                                && rendered.contains(expected_glob.trim_matches('*'))
                        }
                    };
                    if !ok {
                        violations.push(Violation {
                            severity: Severity::Error,
                            rule: "template_value_mismatch".to_string(),
                            message: format!(
                                "Role \"{}\": template rendering ({}) key \"{}\" does not satisfy \"{}\"{}",
                                role.name, src_tag, key, expected_glob,
                                provenance_suffix(r)
                            ),
                            details: Some(format!("{} @ {}", role_tag, path)),
                            expected: Some(format!("{} = {}", key, expected_glob)),
                            actual: Some(
                                actual_val.unwrap_or_else(|| "(not in rendering)".to_string()),
                            ),
                        });
                    } else {
                        *passed += 1;
                    }
                }
            }
        }
    }

    // must_have_file — evaluated once per role (globs may match anywhere in
    // the workspace, not only inside matched resources). We check the union
    // of all resource paths as a cheap proxy for "does the workspace contain
    // a file matching the glob".
    for glob_pat in &role.must_have_file {
        let pat = match glob::Pattern::new(glob_pat) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let any_match = matched.iter().any(|r| {
            r.path
                .as_deref()
                .map(normalize_path)
                .is_some_and(|p| pat.matches(&p))
        });
        if !any_match {
            violations.push(Violation {
                severity: Severity::Warning,
                rule: "role_file_missing".to_string(),
                message: format!(
                    "Role \"{}\": required file pattern \"{}\" not satisfied",
                    role.name, glob_pat
                ),
                details: Some(role_tag.clone()),
                expected: Some(glob_pat.clone()),
                actual: Some("(not found)".to_string()),
            });
        } else {
            *passed += 1;
        }
    }
}

/// Extract container-side port numbers from a docker_service resource's
/// `ports` extra. Mappings like `"127.0.0.1:5090:8080"` yield `8080`.
/// Prefers RFC 006 `resolved_ports` so `${VAR:-5432}` becomes 5432 at check
/// time rather than being silently dropped by the port parser.
fn extract_container_ports(r: &Resource) -> Vec<u16> {
    let ports_source = r
        .extra
        .get("resolved_ports")
        .or_else(|| r.extra.get("ports"));
    let Some(ports) = ports_source.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    ports
        .iter()
        .filter_map(|v| v.as_str())
        .filter_map(|s| {
            // Strip env-var defaults like ${PORT:-8080}
            let last = s.rsplit_once(':').map(|(_, p)| p).unwrap_or(s);
            let cleaned: String = last
                .trim_start_matches('$')
                .trim_start_matches('{')
                .trim_end_matches('}')
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            cleaned.parse::<u16>().ok()
        })
        .collect()
}

/// Extract network names attached to a docker_service resource.
fn extract_networks(r: &Resource) -> Vec<String> {
    r.extra
        .get("networks")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Extract the env var key names defined in an env_file resource.
/// Scanners record them under `extra.keys` (array of strings); fall back to
/// an empty list for types that don't carry the attribute.
fn extract_env_keys(r: &Resource) -> Vec<String> {
    for field in ["keys", "env_keys", "variables_list"] {
        if let Some(arr) = r.extra.get(field).and_then(|v| v.as_array()) {
            return arr
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
        }
    }
    Vec::new()
}

// ─────────────────────────────────────────────────────────
// HTML output
// ─────────────────────────────────────────────────────────

fn render_diff_html(diff: &DiffReport) -> String {
    let status_class = if diff.summary.errors > 0 {
        "fail"
    } else if diff.summary.warnings > 0 {
        "warn"
    } else {
        "pass"
    };
    let status_label = if diff.summary.errors > 0 {
        "VIOLATIONS FOUND"
    } else if diff.summary.warnings > 0 {
        "WARNINGS"
    } else {
        "ALL CHECKS PASSED"
    };

    let mut categories_html = String::new();
    for cat in &diff.categories {
        let error_count = cat
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Error)
            .count();
        let warn_count = cat
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Warning)
            .count();
        let info_count = cat
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Info)
            .count();

        let mut badge_parts = Vec::new();
        if error_count > 0 {
            badge_parts.push(format!(
                r#"<span class="badge badge-error">{} errors</span>"#,
                error_count
            ));
        }
        if warn_count > 0 {
            badge_parts.push(format!(
                r#"<span class="badge badge-warn">{} warnings</span>"#,
                warn_count
            ));
        }
        if info_count > 0 {
            badge_parts.push(format!(
                r#"<span class="badge badge-info">{} info</span>"#,
                info_count
            ));
        }
        if cat.violations.is_empty() && cat.passed > 0 {
            badge_parts.push(format!(
                r#"<span class="badge badge-pass">{} passed</span>"#,
                cat.passed
            ));
        }

        let mut rows = String::new();
        if cat.violations.is_empty() && cat.passed > 0 {
            rows.push_str(&format!(
                r#"<tr class="row-pass"><td><span class="sev sev-pass">PASS</span></td><td colspan="4">All {} checks passed</td></tr>"#,
                cat.passed
            ));
        }
        for v in &cat.violations {
            let (sev_class, sev_label) = match v.severity {
                Severity::Error => ("sev-error", "ERROR"),
                Severity::Warning => ("sev-warn", "WARN"),
                Severity::Info => ("sev-info", "INFO"),
            };
            let detail = v.details.as_deref().unwrap_or("");
            let expected_html = render_diff_cell(v.expected.as_deref(), "del");
            let actual_html = render_diff_cell(v.actual.as_deref(), "ins");
            let message_html = if detail.is_empty() {
                html_escape(&v.message)
            } else {
                format!(
                    "{}<br><span class=\"detail\">{}</span>",
                    html_escape(&v.message),
                    html_escape(detail)
                )
            };
            rows.push_str(&format!(
                r#"<tr><td><span class="sev {}">{}</span></td><td class="rule">{}</td><td class="col-expected">{}</td><td class="col-actual">{}</td><td class="col-msg">{}</td></tr>"#,
                sev_class,
                sev_label,
                html_escape(&v.rule),
                expected_html,
                actual_html,
                message_html,
            ));
        }

        let table_head = if cat.violations.is_empty() {
            String::new()
        } else {
            r#"<thead><tr><th>Sev</th><th>Rule</th><th>Expected (Compositfile)</th><th>Actual (Scan)</th><th>Message</th></tr></thead>"#.to_string()
        };
        categories_html.push_str(&format!(
            r#"
    <div class="category">
      <div class="cat-header">
        <h2>{}</h2>
        <div class="badges">{}</div>
      </div>
      <table>{}<tbody>{}</tbody></table>
    </div>"#,
            html_escape(&cat.name.to_uppercase()),
            badge_parts.join(" "),
            table_head,
            rows
        ));
    }

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>composit diff — {workspace}</title>
<style>
:root {{
  --bg: #0d1117; --surface: #161b22; --border: #30363d;
  --text: #e6edf3; --muted: #7d8590; --accent: #58a6ff;
  --green: #3fb950; --yellow: #d29922; --red: #f85149; --cyan: #79c0ff;
}}
* {{ margin:0; padding:0; box-sizing:border-box; }}
body {{ font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Helvetica,Arial,sans-serif; background:var(--bg); color:var(--text); line-height:1.5; padding:2rem; max-width:960px; margin:0 auto; }}
header {{ border-bottom:1px solid var(--border); padding-bottom:1.5rem; margin-bottom:2rem; }}
header h1 {{ font-size:1.5rem; font-weight:600; color:var(--accent); }}
header h1 span {{ color:var(--muted); font-weight:400; }}
.meta {{ color:var(--muted); font-size:.85rem; margin-top:.5rem; }}
.status {{ display:inline-block; padding:.4rem 1rem; border-radius:8px; font-weight:700; font-size:1rem; margin-top:1rem; letter-spacing:.05em; }}
.status.pass {{ background:rgba(63,185,80,.15); color:var(--green); }}
.status.warn {{ background:rgba(210,153,34,.15); color:var(--yellow); }}
.status.fail {{ background:rgba(248,81,73,.15); color:var(--red); }}
.summary {{ display:flex; gap:1.5rem; margin:1.5rem 0; flex-wrap:wrap; }}
.summary-card {{ background:var(--surface); border:1px solid var(--border); border-radius:8px; padding:1rem 1.5rem; text-align:center; flex:1; min-width:120px; }}
.summary-card .num {{ font-size:2rem; font-weight:700; }}
.summary-card .num.errors {{ color:var(--red); }}
.summary-card .num.warnings {{ color:var(--yellow); }}
.summary-card .num.info {{ color:var(--cyan); }}
.summary-card .num.passed {{ color:var(--green); }}
.summary-card .label {{ font-size:.75rem; color:var(--muted); text-transform:uppercase; letter-spacing:.05em; }}
.category {{ background:var(--surface); border:1px solid var(--border); border-radius:8px; margin-bottom:1rem; overflow:hidden; }}
.cat-header {{ display:flex; justify-content:space-between; align-items:center; padding:.75rem 1rem; border-bottom:1px solid var(--border); background:rgba(13,17,23,.5); }}
.cat-header h2 {{ font-size:.85rem; font-weight:600; letter-spacing:.05em; color:var(--muted); }}
.badges {{ display:flex; gap:.4rem; }}
.badge {{ font-size:.7rem; font-weight:600; padding:.2rem .6rem; border-radius:12px; }}
.badge-error {{ background:rgba(248,81,73,.15); color:var(--red); }}
.badge-warn {{ background:rgba(210,153,34,.15); color:var(--yellow); }}
.badge-info {{ background:rgba(121,192,255,.15); color:var(--cyan); }}
.badge-pass {{ background:rgba(63,185,80,.15); color:var(--green); }}
table {{ width:100%; border-collapse:collapse; font-size:.85rem; }}
td {{ padding:.6rem 1rem; border-bottom:1px solid var(--border); vertical-align:top; }}
tr:last-child td {{ border-bottom:none; }}
.sev {{ display:inline-block; font-weight:700; font-size:.75rem; padding:.15rem .5rem; border-radius:4px; min-width:50px; text-align:center; }}
.sev-error {{ background:rgba(248,81,73,.15); color:var(--red); }}
.sev-warn {{ background:rgba(210,153,34,.15); color:var(--yellow); }}
.sev-info {{ background:rgba(121,192,255,.15); color:var(--cyan); }}
.sev-pass {{ background:rgba(63,185,80,.15); color:var(--green); }}
.rule {{ font-family:'SF Mono',Consolas,monospace; font-size:.8rem; color:var(--muted); white-space:nowrap; }}
.detail {{ color:var(--muted); font-size:.8rem; }}
th {{ text-align:left; padding:.5rem 1rem; color:var(--muted); font-size:.7rem; text-transform:uppercase; letter-spacing:.05em; font-weight:600; border-bottom:1px solid var(--border); background:rgba(13,17,23,.5); }}
.col-expected, .col-actual {{ font-family:'SF Mono',Consolas,monospace; font-size:.8rem; vertical-align:top; max-width:260px; }}
.col-msg {{ font-size:.85rem; vertical-align:top; }}
.scalar {{ display:inline-block; padding:.15rem .5rem; border-radius:4px; }}
.scalar.del {{ background:rgba(248,81,73,.12); color:var(--red); }}
.scalar.ins {{ background:rgba(63,185,80,.12); color:var(--green); }}
.diff-pre {{ font-family:'SF Mono',Consolas,monospace; font-size:.75rem; white-space:pre-wrap; word-break:break-all; margin:0; background:var(--bg); border:1px solid var(--border); border-radius:6px; padding:.5rem; max-height:200px; overflow:auto; }}
.diff-pre .line {{ display:block; padding:0 .25rem; }}
.diff-pre .line.del {{ background:rgba(248,81,73,.12); color:var(--red); }}
.diff-pre .line.ins {{ background:rgba(63,185,80,.12); color:var(--green); }}
.diff-pre .line.del::before {{ content:"- "; opacity:.6; }}
.diff-pre .line.ins::before {{ content:"+ "; opacity:.6; }}
.empty-diff {{ color:var(--muted); font-style:italic; }}
.row-pass td {{ color:var(--green); }}
footer {{ margin-top:2rem; padding-top:1rem; border-top:1px solid var(--border); color:var(--muted); font-size:.8rem; text-align:center; }}
footer a {{ color:var(--accent); text-decoration:none; }}
</style>
</head>
<body>
<header>
  <h1>composit <span>diff</span></h1>
  <div class="meta">Workspace: <strong>{workspace}</strong> &middot; Generated: {generated}</div>
  <div class="status {status_class}">{status_label}</div>
</header>

<div class="summary">
  <div class="summary-card"><div class="num errors">{errors}</div><div class="label">Errors</div></div>
  <div class="summary-card"><div class="num warnings">{warnings}</div><div class="label">Warnings</div></div>
  <div class="summary-card"><div class="num info">{info}</div><div class="label">Info</div></div>
  <div class="summary-card"><div class="num passed">{passed}</div><div class="label">Passed</div></div>
</div>

{categories}

<footer>
  Generated by <a href="https://github.com/nuetzliches/composit">composit</a> diff
</footer>
</body>
</html>"##,
        workspace = html_escape(&diff.workspace),
        generated = html_escape(&diff.generated),
        status_class = status_class,
        status_label = status_label,
        errors = diff.summary.errors,
        warnings = diff.summary.warnings,
        info = diff.summary.info,
        passed = diff.summary.passed_checks,
        categories = categories_html,
    )
}

/// Render the expected/actual cell. Multi-line values become a diff-pre block
/// with +/- line prefixes; scalars become an inline colored chip.
fn render_diff_cell(value: Option<&str>, kind: &str) -> String {
    match value {
        None | Some("") => String::from(r#"<span class="empty-diff">—</span>"#),
        Some(v) if v.contains('\n') => {
            let lines: Vec<String> = v
                .lines()
                .map(|l| format!(r#"<span class="line {}">{}</span>"#, kind, html_escape(l)))
                .collect();
            format!(r#"<pre class="diff-pre">{}</pre>"#, lines.join(""))
        }
        Some(v) => format!(r#"<span class="scalar {}">{}</span>"#, kind, html_escape(v)),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ─────────────────────────────────────────────────────────
// Terminal output
// ─────────────────────────────────────────────────────────

fn print_diff_terminal(diff: &DiffReport) {
    println!();
    println!("{}", "composit diff".bold());
    println!("{}", "=".repeat(60));
    println!("Workspace: {}", diff.workspace);

    for cat in &diff.categories {
        println!();
        let error_count = cat
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Error)
            .count();
        let warn_count = cat
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Warning)
            .count();
        let info_count = cat
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Info)
            .count();

        let mut parts = Vec::new();
        if error_count > 0 {
            parts.push(format!(
                "{} error{}",
                error_count,
                if error_count != 1 { "s" } else { "" }
            ));
        }
        if warn_count > 0 {
            parts.push(format!(
                "{} warning{}",
                warn_count,
                if warn_count != 1 { "s" } else { "" }
            ));
        }
        if info_count > 0 {
            parts.push(format!("{} info", info_count));
        }
        if cat.passed > 0 && cat.violations.is_empty() {
            parts.push("pass".to_string());
        }

        println!(
            "{} ({})",
            cat.name.to_uppercase().bold(),
            if parts.is_empty() {
                "no checks".to_string()
            } else {
                parts.join(", ")
            }
        );

        if cat.violations.is_empty() && cat.passed > 0 {
            println!(
                "  {}  All {} checks passed",
                "PASS".green().bold(),
                cat.passed
            );
        }

        for v in &cat.violations {
            let severity_str = match v.severity {
                Severity::Error => "ERROR".red().bold().to_string(),
                Severity::Warning => "WARN ".yellow().bold().to_string(),
                Severity::Info => "INFO ".cyan().bold().to_string(),
            };
            println!("  {}  {} — {}", severity_str, v.rule.dimmed(), v.message);
            // Show Expected / Actual when present so terminal reaches feature
            // parity with the HTML diff for role_* and resolution violations.
            if let (Some(exp), Some(act)) = (v.expected.as_deref(), v.actual.as_deref()) {
                let exp_display = if exp.contains('\n') {
                    exp.replace('\n', ", ")
                } else {
                    exp.to_string()
                };
                let act_display = if act.contains('\n') {
                    act.replace('\n', ", ")
                } else {
                    act.to_string()
                };
                println!(
                    "         expected {} / actual {}",
                    format!("- {}", exp_display).red(),
                    format!("+ {}", act_display).green()
                );
            }
            if let Some(detail) = &v.details {
                println!("         {}", detail.dimmed());
            }
        }

        // Dedicated resolution-category tail: show env_files_used as a
        // one-line summary so operators see at-a-glance which .env feeds
        // the resolver, without having to open the YAML report.
        if cat.name == "resolution" {
            // Only meaningful when the scan actually opted into resolution;
            // we can't read the report from here, so use a terse placeholder
            // indicator based on category contents.
            let disabled = cat
                .violations
                .iter()
                .any(|v| v.rule == "resolution_disabled");
            if !disabled {
                let unresolved_count = cat
                    .violations
                    .iter()
                    .filter(|v| v.rule == "unresolved_variable")
                    .count();
                if unresolved_count == 0 && cat.passed > 0 {
                    println!("         {}", "(every ${VAR} resolved to a value)".dimmed());
                }
            }
        }
    }

    println!();
    println!("{}", "-".repeat(60));
    println!(
        "  {} | {} | {} | {} passed",
        if diff.summary.errors > 0 {
            format!("{} errors", diff.summary.errors).red().to_string()
        } else {
            "0 errors".to_string()
        },
        if diff.summary.warnings > 0 {
            format!("{} warnings", diff.summary.warnings)
                .yellow()
                .to_string()
        } else {
            "0 warnings".to_string()
        },
        format_args!("{} info", diff.summary.info),
        diff.summary.passed_checks
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::governance::*;
    use crate::core::types::*;

    fn make_report(providers: Vec<Provider>, resources: Vec<Resource>, cost: &str) -> Report {
        let prov_count = providers.len();
        let res_count = resources.len();
        Report {
            workspace: "test".to_string(),
            generated: "2026-04-14".to_string(),
            scanner_version: "0.1.0".to_string(),
            scan_mode: Some(ScanMode::Online),
            providers,
            resources,
            summary: Summary {
                total_resources: res_count,
                providers: prov_count,
                agent_created: 0,
                agent_assisted: 0,
                human_created: res_count,
                auto_detected: 0,
                estimated_monthly_cost: cost.to_string(),
            },
            resolution: None,
        }
    }

    fn make_provider(name: &str) -> Provider {
        Provider {
            name: name.to_string(),
            endpoint: format!("https://{}.example.com", name),
            protocol: "mcp".to_string(),
            capabilities: vec![],
            status: ProviderStatus::Unknown,
            auth_mode: None,
            auth_error: None,
            contract: None,
        }
    }

    fn make_resource(resource_type: &str) -> Resource {
        Resource {
            resource_type: resource_type.to_string(),
            name: None,
            path: Some("./test".to_string()),
            provider: None,
            created: None,
            created_by: None,
            detected_by: "test".to_string(),
            estimated_cost: None,
            extra: std::collections::HashMap::new(),
        }
    }

    /// Look up a category by name — robust against reordering of the fixed
    /// category list in `compute_diff_opts`. Prefer this over `categories[idx]`
    /// in tests so adding a new check at the front doesn't cascade into a
    /// sea of unrelated failures.
    fn find_category<'a>(diff: &'a DiffReport, name: &str) -> &'a ViolationCategory {
        diff.categories
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("diff category '{name}' not present"))
    }

    fn make_governance(providers: Vec<&str>, max_monthly: &str) -> Governance {
        Governance {
            workspace: "test".to_string(),
            providers: providers
                .into_iter()
                .map(|n| ProviderRule {
                    name: n.to_string(),
                    manifest: format!("https://{}.example.com", n),
                    // Tests that don't exercise the contract flow use
                    // "public" to avoid the RFC 002 validation check
                    // that requires an auth block for "contract".
                    trust: "public".to_string(),
                    compliance: vec![],
                    auth: None,
                })
                .collect(),
            budgets: vec![BudgetRule {
                scope: "workspace".to_string(),
                max_monthly: max_monthly.to_string(),
                alert_at: Some("80%".to_string()),
            }],
            policies: vec![],
            resources: None,
            scan: crate::core::governance::ScanSettings::default(),
        }
    }

    #[test]
    fn test_workspace_name_mismatch_surfaces_as_info() {
        // report.workspace (from the scan — Compositfile label, fallback to
        // dirname) must match governance.workspace. A silent drift is
        // confusing — the diff header prints one name while the report shows
        // another. Info severity so it signals without failing CI.
        let mut report = make_report(vec![], vec![], "0 EUR");
        report.workspace = "renamed".to_string();
        let governance = make_governance(vec![], "1000 EUR");

        let diff = compute_diff(&governance, &report, Path::new("."));

        let workspace_cat = diff
            .categories
            .iter()
            .find(|c| c.name == "workspace")
            .expect("workspace category present");
        assert_eq!(workspace_cat.violations.len(), 1);
        let v = &workspace_cat.violations[0];
        assert_eq!(v.rule, "workspace_name_mismatch");
        assert_eq!(v.severity, Severity::Info);
        assert!(v.message.contains("renamed") && v.message.contains("test"));
        assert_eq!(diff.summary.errors, 0);
        assert_eq!(diff.summary.info, 1);
    }

    #[test]
    fn test_workspace_name_match_passes() {
        let report = make_report(vec![], vec![], "0 EUR");
        let governance = make_governance(vec![], "1000 EUR");

        let diff = compute_diff(&governance, &report, Path::new("."));

        let workspace_cat = diff
            .categories
            .iter()
            .find(|c| c.name == "workspace")
            .expect("workspace category present");
        assert!(workspace_cat.violations.is_empty());
        assert_eq!(workspace_cat.passed, 1);
    }

    #[test]
    fn test_unapproved_provider() {
        let report = make_report(
            vec![make_provider("croniq"), make_provider("rogue")],
            vec![],
            "0 EUR",
        );
        let gov = make_governance(vec!["croniq"], "500 EUR");
        let diff = compute_diff(&gov, &report, Path::new("."));

        let prov_cat = find_category(&diff, "providers");
        let errors: Vec<_> = prov_cat
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "unapproved_provider");
        assert!(errors[0].message.contains("rogue"));
    }

    #[test]
    fn test_budget_exceeded() {
        let report = make_report(vec![], vec![], "600 EUR");
        let gov = make_governance(vec![], "500 EUR");
        let diff = compute_diff(&gov, &report, Path::new("."));

        let budget_cat = find_category(&diff, "budgets");
        let errors: Vec<_> = budget_cat
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "budget_exceeded");
    }

    #[test]
    fn test_budget_alert() {
        let report = make_report(vec![], vec![], "420 EUR");
        let gov = make_governance(vec![], "500 EUR");
        let diff = compute_diff(&gov, &report, Path::new("."));

        let budget_cat = find_category(&diff, "budgets");
        let warnings: Vec<_> = budget_cat
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Warning)
            .collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].rule, "budget_alert");
    }

    #[test]
    fn test_resource_max_total_exceeded() {
        let resources: Vec<Resource> = (0..15).map(|_| make_resource("docker_service")).collect();
        let report = make_report(vec![], resources, "0 EUR");
        let mut gov = make_governance(vec![], "500 EUR");
        gov.resources = Some(ResourceConstraints {
            max_total: Some(10),
            allow: vec![],
            require: vec![],
        });

        let diff = compute_diff(&gov, &report, Path::new("."));
        let res_cat = find_category(&diff, "resources");
        let errors: Vec<_> = res_cat
            .violations
            .iter()
            .filter(|v| v.rule == "resource_count_exceeded")
            .collect();
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn test_resource_type_not_allowed() {
        let report = make_report(vec![], vec![make_resource("mcp_server")], "0 EUR");
        let mut gov = make_governance(vec![], "500 EUR");
        gov.resources = Some(ResourceConstraints {
            max_total: None,
            allow: vec![AllowRule {
                resource_type: "docker_service".to_string(),
                max: Some(20),
                allowed_images: vec![],
                allowed_types: vec![],
                roles: vec![],
            }],
            require: vec![],
        });

        let diff = compute_diff(&gov, &report, Path::new("."));
        let res_cat = find_category(&diff, "resources");
        let errors: Vec<_> = res_cat
            .violations
            .iter()
            .filter(|v| v.rule == "resource_type_not_allowed")
            .collect();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("mcp_server"));
    }

    #[test]
    fn test_required_resource_missing() {
        let report = make_report(vec![], vec![], "0 EUR");
        let mut gov = make_governance(vec![], "500 EUR");
        gov.resources = Some(ResourceConstraints {
            max_total: None,
            allow: vec![],
            require: vec![RequireRule {
                resource_type: "docker_compose".to_string(),
                min: 1,
            }],
        });

        let diff = compute_diff(&gov, &report, Path::new("."));
        let res_cat = find_category(&diff, "resources");
        let errors: Vec<_> = res_cat
            .violations
            .iter()
            .filter(|v| v.rule == "required_resource_missing")
            .collect();
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn test_all_checks_pass() {
        let report = make_report(
            vec![make_provider("croniq")],
            vec![
                make_resource("docker_service"),
                make_resource("docker_compose"),
            ],
            "100 EUR",
        );
        let mut gov = make_governance(vec!["croniq"], "500 EUR");
        gov.resources = Some(ResourceConstraints {
            max_total: Some(100),
            allow: vec![
                AllowRule {
                    resource_type: "docker_service".to_string(),
                    max: Some(20),
                    allowed_images: vec![],
                    allowed_types: vec![],
                    roles: vec![],
                },
                AllowRule {
                    resource_type: "docker_compose".to_string(),
                    max: Some(10),
                    allowed_images: vec![],
                    allowed_types: vec![],
                    roles: vec![],
                },
            ],
            require: vec![RequireRule {
                resource_type: "docker_compose".to_string(),
                min: 1,
            }],
        });

        let diff = compute_diff(&gov, &report, Path::new("."));
        assert_eq!(diff.summary.errors, 0);
        assert_eq!(diff.summary.warnings, 0);
        assert!(diff.summary.passed_checks > 0);
    }

    #[test]
    fn test_unused_provider_downgraded_when_offline() {
        // Governance declares a provider; the scan didn't find it.
        // Online: Warning. Offline: Info with an explanatory details field.
        let report = make_report(vec![], vec![], "0 EUR");
        let gov = make_governance(vec!["croniq"], "500 EUR");

        let online = compute_diff_opts(&gov, &report, Path::new("."), false);
        let online_prov = find_category(&online, "providers");
        let online_unused: Vec<_> = online_prov
            .violations
            .iter()
            .filter(|v| v.rule == "unused_provider")
            .collect();
        assert_eq!(online_unused.len(), 1);
        assert_eq!(online_unused[0].severity, Severity::Warning);

        let offline = compute_diff_opts(&gov, &report, Path::new("."), true);
        let offline_prov = find_category(&offline, "providers");
        let offline_unused: Vec<_> = offline_prov
            .violations
            .iter()
            .filter(|v| v.rule == "unused_provider")
            .collect();
        assert_eq!(offline_unused.len(), 1);
        assert_eq!(offline_unused[0].severity, Severity::Info);
        assert!(
            offline_unused[0]
                .details
                .as_deref()
                .is_some_and(|d| d.contains("offline")),
            "offline details should mention the offline mode"
        );
    }

    #[test]
    fn test_parse_percentage_rejects_out_of_range() {
        assert_eq!(parse_percentage("80%"), Some(0.8));
        assert_eq!(parse_percentage("0%"), Some(0.0));
        assert_eq!(parse_percentage("100%"), Some(1.0));
        assert_eq!(parse_percentage("150%"), None);
        assert_eq!(parse_percentage("-10%"), None);
        assert_eq!(parse_percentage("abc"), None);
    }

    // ─────────────────────────────────────────────────────────
    // RFC 002 contract-tier rules
    // ─────────────────────────────────────────────────────────

    fn contract_rule(name: &str) -> ProviderRule {
        ProviderRule {
            name: name.to_string(),
            manifest: format!("https://{}.example.com/.well-known/composit.json", name),
            trust: "contract".to_string(),
            compliance: vec![],
            auth: Some(crate::core::governance::AuthRef {
                auth_type: "api-key".to_string(),
                env: Some("TEST_KEY".to_string()),
            }),
        }
    }

    fn provider_with(auth_mode: Option<AuthMode>, auth_error: Option<&str>) -> Provider {
        let mut p = make_provider("croniq");
        p.auth_mode = auth_mode;
        p.auth_error = auth_error.map(String::from);
        p
    }

    #[test]
    fn contract_success_passes() {
        let rule = contract_rule("croniq");
        let p = provider_with(Some(AuthMode::Contract), None);
        let v = check_provider_contract(&rule, &p, false);
        assert!(v.is_empty(), "contract tier reached: no diagnostics");
    }

    #[test]
    fn contract_auth_missing_is_info() {
        let rule = contract_rule("croniq");
        let p = provider_with(Some(AuthMode::Public), Some("auth_missing"));
        let v = check_provider_contract(&rule, &p, false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "contract_auth_missing");
        assert_eq!(v[0].severity, Severity::Info);
    }

    #[test]
    fn contract_unauthorized_is_error() {
        let rule = contract_rule("croniq");
        let p = provider_with(Some(AuthMode::Public), Some("unauthorized"));
        let v = check_provider_contract(&rule, &p, false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "contract_unauthorized");
        assert_eq!(v[0].severity, Severity::Error);
        assert!(
            v[0].details
                .as_deref()
                .is_some_and(|d| d.contains("TEST_KEY")),
            "error should reference the env var name"
        );
    }

    #[test]
    fn contract_auth_mismatch_is_warning() {
        let rule = contract_rule("croniq");
        let p = provider_with(Some(AuthMode::Public), Some("auth_type_not_advertised"));
        let v = check_provider_contract(&rule, &p, false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "contract_auth_mismatch");
        assert_eq!(v[0].severity, Severity::Warning);
    }

    #[test]
    fn contract_unreachable_from_fetch_error_is_warning() {
        let rule = contract_rule("croniq");
        let p = provider_with(Some(AuthMode::Public), Some("fetch_failed"));
        let v = check_provider_contract(&rule, &p, false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "contract_unreachable");
        assert_eq!(v[0].severity, Severity::Warning);
    }

    #[test]
    fn public_unreachable_always_warns_even_for_public_trust() {
        // unreachable is emitted regardless of trust level.
        let mut rule = contract_rule("croniq");
        rule.trust = "public".to_string();
        rule.auth = None;
        let p = provider_with(Some(AuthMode::Unreachable), Some("fetch_failed"));
        let v = check_provider_contract(&rule, &p, false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "contract_unreachable");
    }

    #[test]
    fn public_trust_never_emits_contract_rules() {
        // trust = public → contract-tier checks are silent even when
        // auth_mode == Public (which is the expected state).
        let mut rule = contract_rule("croniq");
        rule.trust = "public".to_string();
        rule.auth = None;
        let p = provider_with(Some(AuthMode::Public), None);
        let v = check_provider_contract(&rule, &p, false);
        assert!(v.is_empty());
    }

    #[test]
    fn contract_auth_missing_mentions_offline_when_offline() {
        let rule = contract_rule("croniq");
        let p = provider_with(Some(AuthMode::Public), Some("auth_missing"));
        let v = check_provider_contract(&rule, &p, true);
        assert!(
            v[0].details
                .as_deref()
                .is_some_and(|d| d.contains("offline")),
            "offline run should be mentioned in details"
        );
    }

    // ─────────────────────────────────────────────────────────
    // RFC 003 contract_expired + invalid_contract_body
    // ─────────────────────────────────────────────────────────

    fn contract_info(expires_at: &str) -> crate::core::types::ContractInfo {
        crate::core::types::ContractInfo {
            id: "c-test".to_string(),
            issued_at: "2026-01-01T00:00:00Z".to_string(),
            expires_at: expires_at.to_string(),
            pricing_tier: Some("team".to_string()),
            sla: None,
            capabilities: vec![],
        }
    }

    #[test]
    fn contract_expired_past_timestamp_is_error() {
        let rule = contract_rule("croniq");
        let mut p = provider_with(Some(AuthMode::Contract), None);
        p.contract = Some(contract_info("2020-01-01T00:00:00Z"));
        let v = check_provider_contract(&rule, &p, false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "contract_expired");
        assert_eq!(v[0].severity, Severity::Error);
        assert!(
            v[0].message.contains("2020-01-01"),
            "message should quote the expiry timestamp"
        );
    }

    #[test]
    fn contract_expired_future_timestamp_is_silent() {
        let rule = contract_rule("croniq");
        let mut p = provider_with(Some(AuthMode::Contract), None);
        // Far-future date so the test doesn't bitrot.
        p.contract = Some(contract_info("2099-12-31T23:59:59Z"));
        let v = check_provider_contract(&rule, &p, false);
        assert!(v.is_empty(), "future expiry: no diagnostics");
    }

    #[test]
    fn contract_expired_absent_contract_info_is_silent() {
        // Contract-tier reached but scanner didn't populate ContractInfo
        // (back-compat with older reports). Silent until the scanner
        // re-runs and fills in the bookkeeping.
        let rule = contract_rule("croniq");
        let p = provider_with(Some(AuthMode::Contract), None);
        let v = check_provider_contract(&rule, &p, false);
        assert!(v.is_empty());
    }

    #[test]
    fn contract_expired_malformed_timestamp_downgrades_to_unreachable() {
        let rule = contract_rule("croniq");
        let mut p = provider_with(Some(AuthMode::Contract), None);
        p.contract = Some(contract_info("not-a-date"));
        let v = check_provider_contract(&rule, &p, false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "contract_unreachable");
        assert_eq!(v[0].severity, Severity::Warning);
    }

    #[test]
    fn invalid_contract_body_maps_to_unreachable_warning() {
        let rule = contract_rule("croniq");
        let p = provider_with(Some(AuthMode::Public), Some("invalid_contract_body"));
        let v = check_provider_contract(&rule, &p, false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "contract_unreachable");
        assert_eq!(v[0].severity, Severity::Warning);
        assert!(
            v[0].details
                .as_deref()
                .is_some_and(|d| d.contains("RFC 003")
                    || d.contains("contract.provider")
                    || d.contains("contract.{")),
            "details should point at the RFC 003 envelope requirement"
        );
    }

    // ─────────────────────────────────────────────────────────
    // Helper: matches_any_pattern and normalize_path
    // ─────────────────────────────────────────────────────────

    #[test]
    fn matches_any_pattern_literal_and_glob() {
        assert!(matches_any_pattern(
            "postgres:16",
            &["postgres:16".to_string()]
        ));
        assert!(!matches_any_pattern(
            "postgres:latest",
            &["postgres:16".to_string()]
        ));
        assert!(matches_any_pattern("myapp:1.2.3", &["myapp:*".to_string()]));
        assert!(!matches_any_pattern("rogue:1.0", &["myapp:*".to_string()]));
        assert!(matches_any_pattern(
            "registry.example.com/team/app:v2",
            &["registry.example.com/**".to_string()]
        ));
    }

    #[test]
    fn normalize_path_strips_dot_slash_and_backslashes() {
        assert_eq!(normalize_path("./docker-compose.yml"), "docker-compose.yml");
        assert_eq!(normalize_path("services/api.yml"), "services/api.yml");
        assert_eq!(
            normalize_path("services\\api.yml"),
            "services/api.yml",
            "Windows backslashes should be normalised"
        );
    }

    // ─────────────────────────────────────────────────────────
    // Helper: extractor functions
    // ─────────────────────────────────────────────────────────

    fn resource_with_ports(ports: &[&str]) -> Resource {
        let mut r = make_resource("docker_service");
        r.extra.insert(
            "ports".to_string(),
            serde_json::Value::Array(
                ports
                    .iter()
                    .map(|p| serde_json::Value::String(p.to_string()))
                    .collect(),
            ),
        );
        r
    }

    #[test]
    fn extract_ports_bare_number() {
        let r = resource_with_ports(&["8080"]);
        assert_eq!(extract_container_ports(&r), vec![8080u16]);
    }

    #[test]
    fn extract_ports_host_container_mapping() {
        // "host_port:container_port" — only the container side matters.
        let r = resource_with_ports(&["80:8080", "5432:5432"]);
        let ports = extract_container_ports(&r);
        assert!(ports.contains(&8080));
        assert!(ports.contains(&5432));
        assert!(!ports.contains(&80), "host port should not appear");
    }

    #[test]
    fn extract_ports_ip_host_container_mapping() {
        // "ip:host:container" — only the container side matters.
        let r = resource_with_ports(&["127.0.0.1:5090:9000"]);
        let ports = extract_container_ports(&r);
        assert_eq!(ports, vec![9000u16]);
    }

    #[test]
    fn extract_networks_returns_names() {
        let mut r = make_resource("docker_service");
        r.extra.insert(
            "networks".to_string(),
            serde_json::json!(["backend", "monitoring"]),
        );
        assert_eq!(extract_networks(&r), vec!["backend", "monitoring"]);
    }

    #[test]
    fn extract_env_keys_reads_keys_field() {
        let mut r = make_resource("env_file");
        r.extra.insert(
            "keys".to_string(),
            serde_json::json!(["DATABASE_URL", "API_KEY"]),
        );
        assert_eq!(
            extract_env_keys(&r),
            vec!["DATABASE_URL".to_string(), "API_KEY".to_string()]
        );
    }

    #[test]
    fn image_for_matching_prefers_resolved() {
        let mut r = make_resource("docker_service");
        r.extra
            .insert("image".to_string(), serde_json::json!("${APP_IMAGE}"));
        r.extra.insert(
            "resolved_image".to_string(),
            serde_json::json!("myapp:1.2.3"),
        );
        assert_eq!(
            image_for_matching(&r).as_deref(),
            Some("myapp:1.2.3"),
            "resolved_image must take precedence over image template"
        );
    }

    // ─────────────────────────────────────────────────────────
    // RFC 005 — role_matches
    // ─────────────────────────────────────────────────────────

    fn resource_with_image(name: &str, image: &str) -> Resource {
        let mut r = make_resource("docker_service");
        r.name = Some(name.to_string());
        r.extra
            .insert("image".to_string(), serde_json::json!(image));
        r
    }

    #[test]
    fn role_matches_empty_matcher_selects_all() {
        let role = Role {
            name: "any".to_string(),
            ..Default::default()
        };
        assert!(role_matches(&role, &make_resource("docker_service")));
    }

    #[test]
    fn role_matches_by_name_glob() {
        let role = Role {
            name: "db".to_string(),
            matcher: Matcher {
                name: vec!["db-*".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        let mut hit = make_resource("docker_service");
        hit.name = Some("db-primary".to_string());
        let mut miss = make_resource("docker_service");
        miss.name = Some("api".to_string());

        assert!(role_matches(&role, &hit));
        assert!(!role_matches(&role, &miss));
    }

    #[test]
    fn role_matches_by_image_glob() {
        let role = Role {
            name: "postgres".to_string(),
            matcher: Matcher {
                image: vec!["postgres:*".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        let hit = resource_with_image("db", "postgres:16");
        let miss = resource_with_image("api", "myapp:1.0");
        assert!(role_matches(&role, &hit));
        assert!(!role_matches(&role, &miss));
    }

    #[test]
    fn role_matches_any_predicate_uses_or_logic() {
        let role = Role {
            name: "multi".to_string(),
            matcher: Matcher {
                name: vec!["db-*".to_string()],
                image: vec!["redis:*".to_string()],
                predicate: Predicate::Any,
                ..Default::default()
            },
            ..Default::default()
        };
        // name matches, image doesn't → still selected under Any
        let r = resource_with_image("db-replica", "postgres:16");
        assert!(role_matches(&role, &r));
        // neither matches
        let r2 = resource_with_image("api", "myapp:1.0");
        assert!(!role_matches(&role, &r2));
    }

    // ─────────────────────────────────────────────────────────
    // RFC 005 — check_role_constraints
    // ─────────────────────────────────────────────────────────

    fn run_role(role: Role, resources: &[&Resource]) -> (Vec<Violation>, usize) {
        let mut violations = Vec::new();
        let mut passed = 0;
        check_role_constraints(
            &role,
            "docker_service",
            resources,
            &mut violations,
            &mut passed,
        );
        (violations, passed)
    }

    #[test]
    fn role_image_pin_violation_and_pass() {
        let role = Role {
            name: "api".to_string(),
            image_pin: vec!["myapp:1.2.3".to_string()],
            ..Default::default()
        };
        let bad = resource_with_image("api", "myapp:latest");
        let (v, passed) = run_role(role.clone(), &[&bad]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "role_image_not_pinned");
        assert_eq!(passed, 0);

        let good = resource_with_image("api", "myapp:1.2.3");
        let (v, passed) = run_role(role, &[&good]);
        assert!(v.is_empty());
        assert_eq!(passed, 1);
    }

    #[test]
    fn role_image_prefix_mismatch() {
        let role = Role {
            name: "internal".to_string(),
            image_prefix: vec!["registry.internal/".to_string()],
            ..Default::default()
        };
        let bad = resource_with_image("svc", "dockerhub.io/myapp:1.0");
        let (v, _) = run_role(role.clone(), &[&bad]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "role_image_prefix_mismatch");

        let good = resource_with_image("svc", "registry.internal/myapp:1.0");
        let (v, passed) = run_role(role, &[&good]);
        assert!(v.is_empty());
        assert_eq!(passed, 1);
    }

    #[test]
    fn role_min_count_below_minimum() {
        let role = Role {
            name: "replica".to_string(),
            min_count: Some(3),
            ..Default::default()
        };
        let r = resource_with_image("svc", "myapp:1.0");
        let (v, _) = run_role(role, &[&r]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "role_count_below_min");
    }

    #[test]
    fn role_max_count_above_maximum() {
        let role = Role {
            name: "singleton".to_string(),
            max_count: Some(1),
            ..Default::default()
        };
        let r1 = resource_with_image("svc-a", "myapp:1.0");
        let r2 = resource_with_image("svc-b", "myapp:1.0");
        let (v, _) = run_role(role, &[&r1, &r2]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "role_count_above_max");
    }

    #[test]
    fn role_must_expose_port_missing_and_present() {
        let role = Role {
            name: "web".to_string(),
            must_expose: vec![8080],
            ..Default::default()
        };
        let bad = resource_with_ports(&["9000"]);
        let (v, _) = run_role(role.clone(), &[&bad]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "role_port_missing");

        let good = resource_with_ports(&["8080"]);
        let (v, passed) = run_role(role, &[&good]);
        assert!(v.is_empty());
        assert_eq!(passed, 1);
    }

    #[test]
    fn role_must_attach_to_network_missing() {
        let role = Role {
            name: "backend".to_string(),
            must_attach_to: vec!["internal".to_string()],
            ..Default::default()
        };
        let mut r = make_resource("docker_service");
        r.extra
            .insert("networks".to_string(), serde_json::json!(["public"]));
        let (v, _) = run_role(role, &[&r]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "role_network_missing");
    }

    #[test]
    fn role_must_set_env_missing_and_forbidden_present() {
        let role = Role {
            name: "secure".to_string(),
            must_set_env: vec!["DATABASE_URL".to_string()],
            forbidden_env: vec!["DEBUG".to_string()],
            ..Default::default()
        };
        let mut r = make_resource("env_file");
        // Has DEBUG (forbidden) but not DATABASE_URL (required)
        r.extra
            .insert("keys".to_string(), serde_json::json!(["DEBUG", "PORT"]));

        let (v, _) = run_role(role, &[&r]);
        let rules: Vec<&str> = v.iter().map(|x| x.rule.as_str()).collect();
        assert!(
            rules.contains(&"role_env_var_missing"),
            "must_set_env should fire"
        );
        assert!(
            rules.contains(&"role_env_var_forbidden"),
            "forbidden_env should fire"
        );
    }

    #[test]
    fn role_must_have_file_missing() {
        let role = Role {
            name: "api".to_string(),
            must_have_file: vec!["Dockerfile".to_string()],
            ..Default::default()
        };
        // Resource path doesn't satisfy the glob
        let mut r = make_resource("docker_service");
        r.path = Some("docker-compose.yml".to_string());
        let (v, _) = run_role(role, &[&r]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "role_file_missing");
    }

    // ─────────────────────────────────────────────────────────
    // check_resolution (RFC 006)
    // ─────────────────────────────────────────────────────────

    fn report_with_templated_image() -> Report {
        let mut r = make_resource("docker_service");
        r.extra
            .insert("image".to_string(), serde_json::json!("${APP_IMAGE}"));
        make_report(vec![], vec![r], "0 EUR")
    }

    #[test]
    fn resolution_disabled_surfaces_when_templates_present_but_no_resolver() {
        let report = report_with_templated_image();
        let gov = make_governance(vec![], "500 EUR");
        let diff = compute_diff(&gov, &report, Path::new("."));
        let cat = find_category(&diff, "resolution");
        assert!(
            cat.violations
                .iter()
                .any(|v| v.rule == "resolution_disabled"),
            "resolution_disabled must fire when ${{}} templates exist but report.resolution is None"
        );
    }

    #[test]
    fn resolution_no_warning_when_no_templates() {
        let report = make_report(vec![], vec![make_resource("docker_service")], "0 EUR");
        let gov = make_governance(vec![], "500 EUR");
        let diff = compute_diff(&gov, &report, Path::new("."));
        let cat = find_category(&diff, "resolution");
        assert!(
            cat.violations
                .iter()
                .all(|v| v.rule != "resolution_disabled"),
            "no resolution_disabled when no templates present"
        );
    }

    #[test]
    fn resolution_disabled_silenced_when_resolvable_explicitly_empty() {
        // `scan { resolvable = [] }` means "I know, I deliberately opted out".
        // The info must not fire even when ${VAR} refs are present.
        let report = report_with_templated_image();
        let mut gov = make_governance(vec![], "500 EUR");
        gov.scan.resolvable = Some(vec![]); // explicit opt-out
        let diff = compute_diff(&gov, &report, Path::new("."));
        let cat = find_category(&diff, "resolution");
        assert!(
            cat.violations
                .iter()
                .all(|v| v.rule != "resolution_disabled"),
            "resolution_disabled must be silent when resolvable = [] (explicit opt-out)"
        );
    }

    #[test]
    fn resolution_unresolved_variable_surfaces_per_variable() {
        use crate::core::scanner::{ResolutionInfo, UnresolvedVariable};
        let mut report = report_with_templated_image();
        report.resolution = Some(ResolutionInfo {
            env_files_used: vec![".env".to_string()],
            unresolved: vec![UnresolvedVariable {
                variable: "APP_IMAGE".to_string(),
                resource_path: "docker-compose.yml".to_string(),
                field: "image".to_string(),
            }],
        });
        let gov = make_governance(vec![], "500 EUR");
        let diff = compute_diff(&gov, &report, Path::new("."));
        let cat = find_category(&diff, "resolution");
        let unresolved: Vec<_> = cat
            .violations
            .iter()
            .filter(|v| v.rule == "unresolved_variable")
            .collect();
        assert_eq!(unresolved.len(), 1);
        assert!(
            unresolved[0].message.contains("APP_IMAGE"),
            "message should name the unresolved variable"
        );
    }
}
