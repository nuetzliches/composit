use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::cli::DiffOutputFormat;
use crate::core::compositfile::parse_compositfile;
use crate::core::governance::Governance;
use crate::core::types::{Report, ScanMode};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub severity: Severity,
    pub rule: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
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
    let mut categories = Vec::new();

    categories.push(check_providers(governance, report, offline));
    categories.push(check_budgets(governance, report));
    categories.push(check_resources(governance, report));
    categories.push(check_policies(governance, base_dir));

    let errors: usize = categories.iter().flat_map(|c| &c.violations).filter(|v| v.severity == Severity::Error).count();
    let warnings: usize = categories.iter().flat_map(|c| &c.violations).filter(|v| v.severity == Severity::Warning).count();
    let info: usize = categories.iter().flat_map(|c| &c.violations).filter(|v| v.severity == Severity::Info).count();
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

fn check_providers(
    governance: &Governance,
    report: &Report,
    offline: bool,
) -> ViolationCategory {
    let mut violations = Vec::new();
    let mut passed = 0;

    let approved_names: Vec<&str> = governance.providers.iter().map(|p| p.name.as_str()).collect();

    // Check report providers against approved list
    for rp in &report.providers {
        if approved_names.contains(&rp.name.as_str()) {
            passed += 1;
        } else {
            violations.push(Violation {
                severity: Severity::Error,
                rule: "unapproved_provider".to_string(),
                message: format!("Provider \"{}\" found in report but not approved in Compositfile", rp.name),
                details: Some(format!("Endpoint: {}", rp.endpoint)),
            });
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
        });
    }

    ViolationCategory {
        name: "providers".to_string(),
        violations,
        passed,
    }
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
                                report.summary.estimated_monthly_cost, (threshold * 100.0) as usize, alert_amount
                            ),
                            details: None,
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
    let allowed_types: Vec<&str> = constraints.allow.iter().map(|a| a.resource_type.as_str()).collect();

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
                                    "Image \"{}\" not in allowed list for {}",
                                    image, rule.resource_type
                                ),
                                details: r.path.clone(),
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
                                    "Resource type \"{}\" not in allowed types for {}",
                                    rt, rule.resource_type
                                ),
                                details: r.path.clone(),
                            });
                        }
                    }
                }
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

fn check_policies(governance: &Governance, base_dir: &Path) -> ViolationCategory {
    use crate::core::rego::{parse_rego, RegoIssue};

    let mut violations = Vec::new();
    let mut passed = 0;

    for policy in &governance.policies {
        let policy_path = base_dir.join(&policy.source);

        // Only .rego files get parsed. Other source types (a .md file,
        // a link to an external system) are checked for existence only.
        let is_rego = policy_path
            .extension()
            .and_then(|e| e.to_str())
            .map_or(false, |e| e.eq_ignore_ascii_case("rego"));

        if !policy_path.exists() {
            violations.push(Violation {
                severity: Severity::Warning,
                rule: "policy_file_missing".to_string(),
                message: format!(
                    "Policy \"{}\" references missing file: {}",
                    policy.name, policy.source
                ),
                details: None,
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
                    message: format!(
                        "Policy \"{}\" could not be read: {}",
                        policy.name, e
                    ),
                    details: Some(policy.source.clone()),
                });
                continue;
            }
        };

        match parse_rego(&content) {
            Ok(meta) => {
                passed += 1;
                let entrypoints = match (meta.has_default_allow, meta.has_deny) {
                    (true, true) => "allow + deny",
                    (true, false) => "allow",
                    (false, true) => "deny",
                    (false, false) => "none",
                };
                violations.push(Violation {
                    severity: Severity::Info,
                    rule: "policy_parsed".to_string(),
                    message: format!(
                        "Policy \"{}\" ({}): package `{}`, {} rule(s), entrypoints: {}",
                        policy.name,
                        policy.source,
                        meta.package,
                        meta.rules.len(),
                        entrypoints
                    ),
                    details: if meta.rules.is_empty() {
                        None
                    } else {
                        Some(format!("rules: {}", meta.rules.join(", ")))
                    },
                });
            }
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
                });
            }
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
    patterns.iter().any(|p| {
        glob::Pattern::new(p)
            .map_or(false, |pat| pat.matches(value))
    })
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
        let error_count = cat.violations.iter().filter(|v| v.severity == Severity::Error).count();
        let warn_count = cat.violations.iter().filter(|v| v.severity == Severity::Warning).count();
        let info_count = cat.violations.iter().filter(|v| v.severity == Severity::Info).count();

        let mut badge_parts = Vec::new();
        if error_count > 0 { badge_parts.push(format!(r#"<span class="badge badge-error">{} errors</span>"#, error_count)); }
        if warn_count > 0 { badge_parts.push(format!(r#"<span class="badge badge-warn">{} warnings</span>"#, warn_count)); }
        if info_count > 0 { badge_parts.push(format!(r#"<span class="badge badge-info">{} info</span>"#, info_count)); }
        if cat.violations.is_empty() && cat.passed > 0 {
            badge_parts.push(format!(r#"<span class="badge badge-pass">{} passed</span>"#, cat.passed));
        }

        let mut rows = String::new();
        if cat.violations.is_empty() && cat.passed > 0 {
            rows.push_str(&format!(
                r#"<tr class="row-pass"><td><span class="sev sev-pass">PASS</span></td><td colspan="2">All {} checks passed</td></tr>"#,
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
            rows.push_str(&format!(
                r#"<tr><td><span class="sev {}">{}</span></td><td class="rule">{}</td><td>{}{}</td></tr>"#,
                sev_class, sev_label,
                html_escape(&v.rule),
                html_escape(&v.message),
                if detail.is_empty() { String::new() } else { format!(r#"<br><span class="detail">{}</span>"#, html_escape(detail)) }
            ));
        }

        categories_html.push_str(&format!(
            r#"
    <div class="category">
      <div class="cat-header">
        <h2>{}</h2>
        <div class="badges">{}</div>
      </div>
      <table>{}</table>
    </div>"#,
            html_escape(&cat.name.to_uppercase()),
            badge_parts.join(" "),
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
        let error_count = cat.violations.iter().filter(|v| v.severity == Severity::Error).count();
        let warn_count = cat.violations.iter().filter(|v| v.severity == Severity::Warning).count();
        let info_count = cat.violations.iter().filter(|v| v.severity == Severity::Info).count();

        let mut parts = Vec::new();
        if error_count > 0 {
            parts.push(format!("{} error{}", error_count, if error_count != 1 { "s" } else { "" }));
        }
        if warn_count > 0 {
            parts.push(format!("{} warning{}", warn_count, if warn_count != 1 { "s" } else { "" }));
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
            if parts.is_empty() { "no checks".to_string() } else { parts.join(", ") }
        );

        if cat.violations.is_empty() && cat.passed > 0 {
            println!("  {}  All {} checks passed", "PASS".green().bold(), cat.passed);
        }

        for v in &cat.violations {
            let severity_str = match v.severity {
                Severity::Error => "ERROR".red().bold().to_string(),
                Severity::Warning => "WARN ".yellow().bold().to_string(),
                Severity::Info => "INFO ".cyan().bold().to_string(),
            };
            println!("  {}  {} — {}", severity_str, v.rule.dimmed(), v.message);
            if let Some(detail) = &v.details {
                println!("         {}", detail.dimmed());
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
            format!("{} warnings", diff.summary.warnings).yellow().to_string()
        } else {
            "0 warnings".to_string()
        },
        format!("{} info", diff.summary.info),
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
        }
    }

    fn make_provider(name: &str) -> Provider {
        Provider {
            name: name.to_string(),
            endpoint: format!("https://{}.example.com", name),
            protocol: "mcp".to_string(),
            capabilities: vec![],
            status: ProviderStatus::Unknown,
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

    fn make_governance(providers: Vec<&str>, max_monthly: &str) -> Governance {
        Governance {
            workspace: "test".to_string(),
            providers: providers
                .into_iter()
                .map(|n| ProviderRule {
                    name: n.to_string(),
                    manifest: format!("https://{}.example.com", n),
                    trust: "contract".to_string(),
                    compliance: vec![],
                })
                .collect(),
            budgets: vec![BudgetRule {
                scope: "workspace".to_string(),
                max_monthly: max_monthly.to_string(),
                alert_at: Some("80%".to_string()),
            }],
            policies: vec![],
            resources: None,
        }
    }

    #[test]
    fn test_unapproved_provider() {
        let report = make_report(vec![make_provider("croniq"), make_provider("rogue")], vec![], "0 EUR");
        let gov = make_governance(vec!["croniq"], "500 EUR");
        let diff = compute_diff(&gov, &report, Path::new("."));

        let prov_cat = &diff.categories[0];
        assert_eq!(prov_cat.name, "providers");
        let errors: Vec<_> = prov_cat.violations.iter().filter(|v| v.severity == Severity::Error).collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "unapproved_provider");
        assert!(errors[0].message.contains("rogue"));
    }

    #[test]
    fn test_budget_exceeded() {
        let report = make_report(vec![], vec![], "600 EUR");
        let gov = make_governance(vec![], "500 EUR");
        let diff = compute_diff(&gov, &report, Path::new("."));

        let budget_cat = &diff.categories[1];
        let errors: Vec<_> = budget_cat.violations.iter().filter(|v| v.severity == Severity::Error).collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "budget_exceeded");
    }

    #[test]
    fn test_budget_alert() {
        let report = make_report(vec![], vec![], "420 EUR");
        let gov = make_governance(vec![], "500 EUR");
        let diff = compute_diff(&gov, &report, Path::new("."));

        let budget_cat = &diff.categories[1];
        let warnings: Vec<_> = budget_cat.violations.iter().filter(|v| v.severity == Severity::Warning).collect();
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
        let res_cat = &diff.categories[2];
        let errors: Vec<_> = res_cat.violations.iter().filter(|v| v.rule == "resource_count_exceeded").collect();
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
            }],
            require: vec![],
        });

        let diff = compute_diff(&gov, &report, Path::new("."));
        let res_cat = &diff.categories[2];
        let errors: Vec<_> = res_cat.violations.iter().filter(|v| v.rule == "resource_type_not_allowed").collect();
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
        let res_cat = &diff.categories[2];
        let errors: Vec<_> = res_cat.violations.iter().filter(|v| v.rule == "required_resource_missing").collect();
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn test_all_checks_pass() {
        let report = make_report(
            vec![make_provider("croniq")],
            vec![make_resource("docker_service"), make_resource("docker_compose")],
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
                },
                AllowRule {
                    resource_type: "docker_compose".to_string(),
                    max: Some(10),
                    allowed_images: vec![],
                    allowed_types: vec![],
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
        let online_prov = &online.categories[0];
        let online_unused: Vec<_> = online_prov
            .violations
            .iter()
            .filter(|v| v.rule == "unused_provider")
            .collect();
        assert_eq!(online_unused.len(), 1);
        assert_eq!(online_unused[0].severity, Severity::Warning);

        let offline = compute_diff_opts(&gov, &report, Path::new("."), true);
        let offline_prov = &offline.categories[0];
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
                .map_or(false, |d| d.contains("offline")),
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
}
