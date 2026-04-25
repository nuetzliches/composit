//! Cargo manifest scanner.
//!
//! Surfaces `Cargo.toml` files as governance-relevant artifacts: a
//! workspace root declares which crates ship together; each `[package]`
//! manifest declares the binary/library that may end up deployed.
//!
//! Resource types:
//! - `cargo_workspace` for files declaring `[workspace]` (member list)
//! - `cargo_crate` for files declaring `[package]` (name, version, edition)
//!
//! A single Cargo.toml can declare both — composit emits one resource of
//! each in that case so downstream rules can target either independently.
//!
//! Hand-rolled parsing on top of the line stream avoids pulling in the
//! `toml` crate for what is currently a small, well-shaped surface. The
//! cost is correctness on exotic TOML (multi-line arrays of inline
//! tables, etc.) — see tests for the supported shapes.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct CargoManifestScanner;

#[async_trait]
impl Scanner for CargoManifestScanner {
    fn id(&self) -> &str {
        "cargo_manifest"
    }

    fn name(&self) -> &str {
        "Cargo Manifest Scanner"
    }

    fn description(&self) -> &str {
        "Detects Cargo.toml — workspaces, member crates, package metadata"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        let pattern = context.dir.join("**/Cargo.toml");
        for entry in glob(&pattern.to_string_lossy())?.flatten() {
            if context.is_excluded(&entry) || !entry.is_file() {
                continue;
            }
            // Skip Cargo.toml inside target/ — those are vendored snapshots,
            // not first-party governance.
            if entry.components().any(|c| c.as_os_str() == "target") {
                continue;
            }
            resources.extend(parse_manifest(&entry, &context.dir));
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
            resolution: None,
        })
    }
}

#[derive(Default, Debug)]
struct Sections {
    workspace_members: Vec<String>,
    workspace_version: Option<String>,
    package_name: Option<String>,
    package_version: Option<String>,
    package_edition: Option<String>,
    package_license: Option<String>,
    has_workspace: bool,
    has_package: bool,
}

fn parse_manifest(path: &Path, base_dir: &Path) -> Vec<Resource> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let sections = parse_sections(&content);
    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let path_str = format!("./{rel}");

    let mut out = Vec::new();
    if sections.has_workspace {
        let mut extra = HashMap::new();
        extra.insert(
            "members".to_string(),
            serde_json::Value::Array(
                sections
                    .workspace_members
                    .iter()
                    .map(|m| serde_json::Value::String(m.clone()))
                    .collect(),
            ),
        );
        extra.insert(
            "member_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(sections.workspace_members.len())),
        );
        if let Some(v) = &sections.workspace_version {
            extra.insert("version".to_string(), serde_json::Value::String(v.clone()));
        }
        out.push(Resource {
            resource_type: "cargo_workspace".to_string(),
            name: path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string()),
            path: Some(path_str.clone()),
            provider: None,
            created: None,
            created_by: None,
            detected_by: "cargo_manifest".to_string(),
            estimated_cost: None,
            extra,
        });
    }
    if sections.has_package {
        let mut extra = HashMap::new();
        if let Some(v) = &sections.package_version {
            extra.insert("version".to_string(), serde_json::Value::String(v.clone()));
        }
        if let Some(e) = &sections.package_edition {
            extra.insert("edition".to_string(), serde_json::Value::String(e.clone()));
        }
        if let Some(l) = &sections.package_license {
            extra.insert("license".to_string(), serde_json::Value::String(l.clone()));
        }
        out.push(Resource {
            resource_type: "cargo_crate".to_string(),
            name: sections.package_name.clone(),
            path: Some(path_str),
            provider: None,
            created: None,
            created_by: None,
            detected_by: "cargo_manifest".to_string(),
            estimated_cost: None,
            extra,
        });
    }
    out
}

fn parse_sections(content: &str) -> Sections {
    let mut sec = Sections::default();
    // current section header — `[workspace]`, `[package]`, etc. We only
    // populate fields when in the relevant section, so a `version` under
    // `[workspace.package]` doesn't get mistaken for `[package].version`.
    let mut current = String::new();
    let mut in_members_array = false;

    for raw in content.lines() {
        let line = raw.trim();

        // Section header.
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            current = name.to_string();
            in_members_array = false;
            if current == "workspace" {
                sec.has_workspace = true;
            } else if current == "package" {
                sec.has_package = true;
            }
            continue;
        }

        // Multi-line `members = [` array continuation.
        if in_members_array {
            if line.starts_with(']') {
                in_members_array = false;
                continue;
            }
            if let Some(member) = strip_array_string(line) {
                sec.workspace_members.push(member);
            }
            continue;
        }

        // key = value lines (only relevant under tracked sections).
        let (key, value) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => continue,
        };

        match (current.as_str(), key) {
            ("workspace", "members") => {
                // Single-line `members = ["a", "b"]` or multi-line opener.
                if let Some(rest) = value.strip_prefix('[') {
                    // Try to consume members on the same line.
                    let trimmed = rest.trim_end_matches(']').trim();
                    if !trimmed.is_empty() {
                        for chunk in trimmed.split(',') {
                            if let Some(m) = strip_array_string(chunk.trim()) {
                                sec.workspace_members.push(m);
                            }
                        }
                    }
                    if !rest.contains(']') {
                        in_members_array = true;
                    }
                }
            }
            ("workspace.package", "version") => {
                sec.workspace_version = Some(strip_string(value));
            }
            ("package", "name") => {
                sec.package_name = Some(strip_string(value));
            }
            ("package", "version") => {
                sec.package_version = Some(strip_string(value));
            }
            ("package", "edition") => {
                sec.package_edition = Some(strip_string(value));
            }
            ("package", "license") => {
                sec.package_license = Some(strip_string(value));
            }
            _ => {}
        }
    }

    sec
}

fn strip_string(value: &str) -> String {
    let stripped = value
        .trim_end_matches('#')
        .split('#')
        .next()
        .unwrap_or(value)
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();
    stripped
}

fn strip_array_string(s: &str) -> Option<String> {
    let trimmed = s.trim().trim_end_matches(',').trim();
    if trimmed.is_empty() {
        return None;
    }
    let unquoted = trimmed.trim_matches('"').trim_matches('\'').to_string();
    if unquoted.is_empty() {
        None
    } else {
        Some(unquoted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parses_workspace_with_multiline_members() {
        let src = r#"
[workspace]
resolver = "2"
members = [
    "crates/foo",
    "crates/bar",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
"#;
        let sec = parse_sections(src);
        assert!(sec.has_workspace);
        assert!(!sec.has_package);
        assert_eq!(sec.workspace_members, vec!["crates/foo", "crates/bar"]);
        assert_eq!(sec.workspace_version.as_deref(), Some("0.1.0"));
    }

    #[test]
    fn parses_workspace_inline_members() {
        let src = r#"
[workspace]
members = ["a", "b", "c"]
"#;
        let sec = parse_sections(src);
        assert_eq!(sec.workspace_members, vec!["a", "b", "c"]);
    }

    #[test]
    fn parses_package_metadata() {
        let src = r#"
[package]
name = "composit"
version = "0.3.2"
edition = "2021"
license = "MIT"
"#;
        let sec = parse_sections(src);
        assert!(sec.has_package);
        assert_eq!(sec.package_name.as_deref(), Some("composit"));
        assert_eq!(sec.package_version.as_deref(), Some("0.3.2"));
        assert_eq!(sec.package_edition.as_deref(), Some("2021"));
        assert_eq!(sec.package_license.as_deref(), Some("MIT"));
    }

    #[test]
    fn workspace_package_version_does_not_leak_into_package_section() {
        // [workspace.package].version is the workspace inheritance default,
        // not a [package] field. Mixed-up parsing would put it in package_version.
        let src = r#"
[workspace]
members = ["a"]

[workspace.package]
version = "0.5.0"
"#;
        let sec = parse_sections(src);
        assert_eq!(sec.workspace_version.as_deref(), Some("0.5.0"));
        assert!(sec.package_version.is_none());
        assert!(!sec.has_package);
    }

    #[test]
    fn manifest_with_both_workspace_and_package_yields_two_resources() {
        let src = r#"
[workspace]
members = ["sub"]

[package]
name = "root-bin"
version = "0.1.0"
"#;
        let dir = tempdir().unwrap();
        let p = dir.path().join("Cargo.toml");
        fs::write(&p, src).unwrap();

        let resources = parse_manifest(&p, dir.path());
        assert_eq!(resources.len(), 2);
        let types: Vec<&str> = resources.iter().map(|r| r.resource_type.as_str()).collect();
        assert!(types.contains(&"cargo_workspace"));
        assert!(types.contains(&"cargo_crate"));
    }
}
