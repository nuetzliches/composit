//! Lightweight Rego (.rego) metadata extraction.
//!
//! Not a full Rego interpreter — composit only needs to answer:
//!
//! - Is this file parseable at the line level (no obvious truncation,
//!   a package declaration, balanced braces)?
//! - What package does it declare?
//! - Which top-level rules does it define, and is there an `allow` /
//!   `deny` entrypoint?
//!
//! This is enough to turn a `policy` block in the Compositfile from
//! "file exists on disk" into "file exists and declares package X with
//! N rules, including an allow/deny entrypoint". Runtime evaluation
//! against scan-derived inputs is deliberately out of scope — the
//! Rego files in real repos (e.g. powerbrain's `opa-policies/pb/`)
//! expect request-shaped inputs, not composit reports, so evaluating
//! them here would produce misleading results.

/// What a `.rego` file declares. Extracted syntactically, so this is
/// "the author's stated intent", not the runtime semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegoMetadata {
    pub package: String,
    pub rules: Vec<String>,
    pub has_default_allow: bool,
    pub has_deny: bool,
}

/// Reason a `.rego` file couldn't be summarised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegoIssue {
    MissingPackage,
    UnbalancedBraces { open: usize, close: usize },
    Empty,
}

impl std::fmt::Display for RegoIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegoIssue::MissingPackage => write!(f, "no `package` declaration found"),
            RegoIssue::UnbalancedBraces { open, close } => write!(
                f,
                "unbalanced braces: {} opening vs. {} closing",
                open, close
            ),
            RegoIssue::Empty => write!(f, "file is empty or contains only comments"),
        }
    }
}

/// Parse a `.rego` file's textual structure. Returns metadata on success
/// or a structural issue on failure. This is a best-effort syntactic
/// scan — it does NOT replace `opa parse`.
pub fn parse_rego(content: &str) -> Result<RegoMetadata, RegoIssue> {
    let mut package: Option<String> = None;
    let mut rules: Vec<String> = Vec::new();
    let mut has_default_allow = false;
    let mut has_deny = false;
    let mut non_comment_lines = 0usize;

    for raw_line in content.lines() {
        let line = strip_inline_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        non_comment_lines += 1;

        // package foo.bar.baz
        if let Some(rest) = line.strip_prefix("package ") {
            if package.is_none() {
                package = Some(rest.trim().to_string());
            }
            continue;
        }

        // Skip imports / meta
        if line.starts_with("import ") {
            continue;
        }

        // default allow := false
        // default allow = false
        if let Some(rest) = line.strip_prefix("default ") {
            let rule = rest
                .split(|c: char| c == ':' || c == '=' || c.is_whitespace())
                .find(|s| !s.is_empty());
            if let Some(r) = rule {
                push_unique(&mut rules, r);
                if r == "allow" {
                    has_default_allow = true;
                }
                if r == "deny" {
                    has_deny = true;
                }
            }
            continue;
        }

        // Rule-like lines: `name := ...`, `name = ...`, `name if {`,
        // `name[x] := ...`, `name(x) = ...`
        if let Some(name) = extract_rule_name(line) {
            push_unique(&mut rules, &name);
            if name == "deny" {
                has_deny = true;
            }
            if name == "allow" && !has_default_allow {
                // A plain `allow if { … }` still counts as an entrypoint
                // but we report has_default_allow only when `default` is set.
            }
        }
    }

    if non_comment_lines == 0 {
        return Err(RegoIssue::Empty);
    }

    let (open, close) = count_braces(content);
    if open != close {
        return Err(RegoIssue::UnbalancedBraces { open, close });
    }

    let package = match package {
        Some(p) => p,
        None => return Err(RegoIssue::MissingPackage),
    };

    Ok(RegoMetadata {
        package,
        rules,
        has_default_allow,
        has_deny,
    })
}

fn push_unique(rules: &mut Vec<String>, name: &str) {
    let s = name.to_string();
    if !rules.contains(&s) {
        rules.push(s);
    }
}

/// Extract the rule name from a line. Returns None if the line doesn't
/// look like a top-level rule declaration.
fn extract_rule_name(line: &str) -> Option<String> {
    // A rule line starts with an identifier followed by `:=`, `=`, `if`,
    // `[`, `(`, or whitespace-then-`if`.
    let first_token: String = line
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    if first_token.is_empty() {
        return None;
    }

    let rest = &line[first_token.len()..];
    let rest_trim = rest.trim_start();

    // Strong markers that this really is a rule head, not arbitrary text.
    let is_rule = rest_trim.starts_with(":=")
        || rest_trim.starts_with('=')
        || rest_trim.starts_with("if")
        || rest_trim.starts_with(" if")
        || rest_trim.starts_with('[')
        || rest_trim.starts_with('(')
        || rest_trim.starts_with("contains ")
        || rest_trim.starts_with('{');

    if !is_rule {
        return None;
    }

    // Filter out Rego keywords that would look like rules but aren't.
    const KEYWORDS: &[&str] = &[
        "package", "import", "default", "else", "not", "with", "as", "in", "some", "every", "true",
        "false", "null",
    ];
    if KEYWORDS.contains(&first_token.as_str()) {
        return None;
    }

    Some(first_token)
}

fn strip_inline_comment(line: &str) -> &str {
    // Conservative: only strip `# …` that's outside of a string literal.
    // For metadata extraction the common case is enough — full lexing is
    // out of scope.
    if let Some(idx) = line.find('#') {
        let before = &line[..idx];
        // Avoid stripping inside a string: if an odd number of "
        // appeared before #, we're inside a string — leave as is.
        if before.matches('"').count().is_multiple_of(2) {
            return before;
        }
    }
    line
}

fn count_braces(content: &str) -> (usize, usize) {
    let mut open = 0usize;
    let mut close = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for c in content.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => open += 1,
            '}' if !in_string => close += 1,
            _ => {}
        }
    }
    (open, close)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_powerbrain_style_policy() {
        let src = r#"
package pb.access

import rego.v1

default allow := false

allow if {
    input.action != "write"
    some role in data.pb.config.access_matrix[input.classification]
    input.agent_role == role
}

allow if {
    input.action == "write"
    some role in data.pb.config.write_roles
    input.agent_role == role
}
"#;
        let meta = parse_rego(src).expect("valid rego");
        assert_eq!(meta.package, "pb.access");
        assert!(meta.has_default_allow);
        assert!(!meta.has_deny);
        assert!(meta.rules.iter().any(|r| r == "allow"));
    }

    #[test]
    fn detects_deny_rules() {
        let src = r#"
package ops.security

default deny := false

deny if {
    input.image == "latest"
}
"#;
        let meta = parse_rego(src).expect("valid");
        assert!(meta.has_deny);
        assert!(meta.rules.iter().any(|r| r == "deny"));
    }

    #[test]
    fn missing_package_is_an_issue() {
        let src = "allow := true\n";
        assert_eq!(parse_rego(src), Err(RegoIssue::MissingPackage));
    }

    #[test]
    fn empty_file_is_an_issue() {
        assert_eq!(parse_rego(""), Err(RegoIssue::Empty));
        assert_eq!(parse_rego("# just a comment\n"), Err(RegoIssue::Empty));
    }

    #[test]
    fn unbalanced_braces_are_detected() {
        let src = r#"
package x
allow if {
    true
"#;
        assert!(matches!(
            parse_rego(src),
            Err(RegoIssue::UnbalancedBraces { .. })
        ));
    }

    #[test]
    fn imports_and_comments_dont_count_as_rules() {
        let src = r#"
# top-level comment
package foo.bar
import rego.v1
import data.ops.util as u

# the rule
something := 42
"#;
        let meta = parse_rego(src).expect("valid");
        assert_eq!(meta.package, "foo.bar");
        assert_eq!(meta.rules, vec!["something"]);
    }

    #[test]
    fn braces_in_strings_dont_confuse_counter() {
        let src = r#"
package x
allow if {
    input.message == "a { character"
}
"#;
        parse_rego(src).expect("balanced once strings are ignored");
    }
}
