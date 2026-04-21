use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct OpaPolicyScanner;

#[async_trait]
impl Scanner for OpaPolicyScanner {
    fn id(&self) -> &str {
        "opa_policy"
    }

    fn name(&self) -> &str {
        "OPA Policy Scanner"
    }

    fn description(&self) -> &str {
        "Detects .rego policy files — package, rule count, deny/allow entrypoints"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        let pattern = context.dir.join("**/*.rego");
        for entry in glob(&pattern.to_string_lossy())?.flatten() {
            if context.is_excluded(&entry) || !entry.is_file() {
                continue;
            }
            if let Some(r) = parse_rego_file(&entry, &context.dir) {
                resources.push(r);
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}

fn parse_rego_file(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;
    let package = extract_package(&content)?;

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let (rule_count, entrypoints) = summarise(&content);

    let mut extra = HashMap::new();
    extra.insert(
        "package".to_string(),
        serde_json::Value::String(package.clone()),
    );
    extra.insert(
        "rules".to_string(),
        serde_json::Value::Number(serde_json::Number::from(rule_count)),
    );
    if !entrypoints.is_empty() {
        extra.insert(
            "entrypoints".to_string(),
            serde_json::Value::Array(
                entrypoints
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }

    Some(Resource {
        resource_type: "opa_policy".to_string(),
        name: Some(package),
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "opa_policy".to_string(),
        estimated_cost: None,
        extra,
    })
}

/// Every valid Rego file starts with `package <dotted.name>`. Without it the
/// OPA compiler rejects the file outright, so using its presence as the
/// "is this really Rego" gate avoids false positives from stray .rego
/// extensions in test fixtures or tutorial material.
fn extract_package(content: &str) -> Option<String> {
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("package ") {
            let name = rest.trim().trim_end_matches(';').trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Counts rule heads (`name {`, `name =`, `name[_]`) and picks out the
/// conventional entrypoints (`allow`, `deny`, `violation`) so downstream
/// governance can spot which policies have an enforcement decision.
fn summarise(content: &str) -> (usize, Vec<String>) {
    let mut rules = 0usize;
    let mut entrypoints: Vec<String> = Vec::new();
    let known_entry = ["allow", "deny", "violation"];

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("package ")
            || line.starts_with("import ")
        {
            continue;
        }

        // Rule heads: `name {`, `name = value {`, `name := value`, `name[x]`
        // The first token before whitespace / `{` / `=` / `[` is the name.
        let head = line
            .split(|c: char| c == '{' || c == '=' || c == '[' || c == '(' || c.is_whitespace())
            .next()
            .unwrap_or("");
        if head.is_empty() || !head.chars().next().unwrap_or(' ').is_alphabetic() {
            continue;
        }
        // Guard: body lines like `some_var := input.x` shouldn't count as
        // a new rule. Heuristic: the line must either open a block (`{`)
        // or be a `default` declaration.
        if line.contains('{') || line.starts_with("default ") {
            rules += 1;
            if known_entry.contains(&head) && !entrypoints.contains(&head.to_string()) {
                entrypoints.push(head.to_string());
            }
        }
    }

    (rules, entrypoints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_package_finds_name() {
        let src = "# header\n\npackage pb.access\n\ndefault allow = false\n";
        assert_eq!(extract_package(src), Some("pb.access".to_string()));
    }

    #[test]
    fn extract_package_none_for_non_rego() {
        assert_eq!(extract_package("// javascript\nconsole.log('hi');\n"), None);
    }

    #[test]
    fn summarise_counts_rules_and_entrypoints() {
        let src = r#"
package pb.access

default allow = false

allow {
    input.role == "admin"
}

deny[msg] {
    input.user == ""
    msg := "empty user"
}

is_admin {
    input.role == "admin"
}
"#;
        let (rules, entries) = summarise(src);
        // default allow, allow {...}, deny[...] {...}, is_admin {...} = 4 rules
        assert_eq!(rules, 4);
        assert!(entries.contains(&"allow".to_string()));
        assert!(entries.contains(&"deny".to_string()));
        assert!(!entries.contains(&"is_admin".to_string()));
    }

    #[test]
    fn summarise_ignores_imports_and_comments() {
        let src = "package x\n\nimport future.keywords.in\n\n# comment rule { }\n";
        let (rules, entries) = summarise(src);
        assert_eq!(rules, 0);
        assert!(entries.is_empty());
    }
}
