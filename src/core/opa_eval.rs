//! OPA/Rego runtime evaluation against composit scan inputs.
//!
//! Policies receive the full scan report as `input`:
//!
//! ```rego
//! package composit.checks
//!
//! import rego.v1
//!
//! deny contains msg if {
//!     some r in input.resources
//!     r.type == "docker_service"
//!     endswith(r.image, ":latest")
//!     msg := sprintf("docker service %v uses :latest tag", [r.name])
//! }
//! ```
//!
//! `input.resources` entries match the JSON serialisation of `Resource`:
//! top-level fields (`type`, `name`, `path`, `detected_by`) plus all
//! `extra` fields flattened in (so `r.image` rather than `r.extra.image`).

use anyhow::{Context, Result};
use regorus::{Engine, Value};

/// What a single policy evaluation returned.
#[derive(Debug)]
pub enum PolicyOutcome {
    /// No violations — policy passed cleanly.
    Clean,
    /// `deny` rules fired; each element is a violation message.
    Denials(Vec<String>),
    /// `allow` evaluated to false (no specific message from the policy).
    NotAllowed,
    /// Regorus could not compile or evaluate the policy.
    EvalError(String),
}

/// Evaluate a `.rego` file against a JSON-serialised composit input.
///
/// `filename`      — path label for error messages (not read from disk here).
/// `rego_source`   — raw Rego source text.
/// `package_path`  — dot-separated OPA package, e.g. `"composit.checks"`.
/// `has_deny`      — policy has at least one `deny` rule.
/// `has_allow`     — policy has a `default allow` declaration.
/// `input_json`    — serialised `Report` (the full scan result as JSON).
pub fn eval_policy(
    filename: &str,
    rego_source: &str,
    package_path: &str,
    has_deny: bool,
    has_allow: bool,
    input_json: &str,
) -> PolicyOutcome {
    match try_eval(filename, rego_source, package_path, has_deny, has_allow, input_json) {
        Ok(o) => o,
        Err(e) => PolicyOutcome::EvalError(e.to_string()),
    }
}

fn try_eval(
    filename: &str,
    rego_source: &str,
    package_path: &str,
    has_deny: bool,
    has_allow: bool,
    input_json: &str,
) -> Result<PolicyOutcome> {
    let mut engine = Engine::new();
    engine
        .add_policy(filename.to_string(), rego_source.to_string())
        .with_context(|| format!("failed to compile {filename}"))?;
    engine
        .set_input_json(input_json)
        .context("failed to set policy input")?;

    // Prefer deny-set semantics. `deny contains msg` is an incremental
    // partial rule — eval_query returns the accumulated set correctly where
    // eval_rule does not support partial rules.
    if has_deny {
        let query = format!("data.{package_path}.deny");
        let results = engine
            .eval_query(query.clone(), false)
            .with_context(|| format!("failed to evaluate {query}"))?;

        let messages = extract_from_query_results(&results);
        if !messages.is_empty() {
            return Ok(PolicyOutcome::Denials(messages));
        }
    }

    // Fall back to allow semantics: a policy that only declares `allow`
    // fails as a unit when allow evaluates to false.
    if has_allow {
        let query = format!("data.{package_path}.allow");
        let allowed = engine
            .eval_bool_query(query.clone(), false)
            .with_context(|| format!("failed to evaluate {query}"))?;

        if !allowed {
            return Ok(PolicyOutcome::NotAllowed);
        }
    }

    Ok(PolicyOutcome::Clean)
}

fn extract_from_query_results(results: &regorus::QueryResults) -> Vec<String> {
    let mut out = Vec::new();
    for qr in &results.result {
        for expr in &qr.expressions {
            collect_strings(&expr.value, &mut out);
        }
    }
    out
}

/// Pull string values out of a Rego Value into `out`: handles Set, Array, bare String.
fn collect_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Set(set) => {
            for item in set.iter() {
                if let Value::String(s) = item {
                    out.push(s.to_string());
                }
            }
        }
        Value::Array(arr) => {
            for item in arr.iter() {
                if let Value::String(s) = item {
                    out.push(s.to_string());
                }
            }
        }
        Value::String(s) => out.push(s.to_string()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DENY_POLICY: &str = r#"
package composit.test

import rego.v1

deny contains msg if {
    some r in input.resources
    r.type == "docker_service"
    endswith(r.image, ":latest")
    msg := sprintf("service %v pins to :latest", [r.name])
}
"#;

    const ALLOW_POLICY: &str = r#"
package composit.allow_test

import rego.v1

default allow := false

allow if {
    every r in input.resources {
        r.type != "docker_service"
    }
}
"#;

    fn input_with_latest() -> String {
        serde_json::json!({
            "workspace": "test",
            "resources": [
                {"type": "docker_service", "name": "api", "image": "myapp:latest", "detected_by": "docker"},
                {"type": "docker_service", "name": "db", "image": "postgres:16", "detected_by": "docker"}
            ]
        })
        .to_string()
    }

    fn input_clean() -> String {
        serde_json::json!({
            "workspace": "test",
            "resources": [
                {"type": "docker_service", "name": "api", "image": "myapp:1.2.3", "detected_by": "docker"}
            ]
        })
        .to_string()
    }

    #[test]
    fn deny_fires_for_latest_tag() {
        let outcome = eval_policy(
            "test.rego",
            DENY_POLICY,
            "composit.test",
            true,
            false,
            &input_with_latest(),
        );
        match outcome {
            PolicyOutcome::Denials(msgs) => {
                assert_eq!(msgs.len(), 1);
                assert!(msgs[0].contains("api"), "message should name the service");
            }
            other => panic!("expected Denials, got {:?}", other),
        }
    }

    #[test]
    fn deny_clean_for_pinned_image() {
        let outcome = eval_policy(
            "test.rego",
            DENY_POLICY,
            "composit.test",
            true,
            false,
            &input_clean(),
        );
        assert!(matches!(outcome, PolicyOutcome::Clean));
    }

    #[test]
    fn allow_returns_not_allowed_when_docker_services_present() {
        let outcome = eval_policy(
            "allow.rego",
            ALLOW_POLICY,
            "composit.allow_test",
            false,
            true,
            &input_with_latest(),
        );
        assert!(matches!(outcome, PolicyOutcome::NotAllowed));
    }

    #[test]
    fn allow_passes_when_no_docker_services() {
        let input = serde_json::json!({
            "workspace": "test",
            "resources": [
                {"type": "terraform_resource", "name": "bucket", "detected_by": "terraform"}
            ]
        })
        .to_string();

        let outcome = eval_policy(
            "allow.rego",
            ALLOW_POLICY,
            "composit.allow_test",
            false,
            true,
            &input,
        );
        assert!(matches!(outcome, PolicyOutcome::Clean));
    }

#[test]
    fn bad_rego_returns_eval_error() {
        let outcome = eval_policy(
            "bad.rego",
            "this is not valid rego syntax !!!",
            "composit.bad",
            true,
            false,
            "{}",
        );
        assert!(matches!(outcome, PolicyOutcome::EvalError(_)));
    }
}
