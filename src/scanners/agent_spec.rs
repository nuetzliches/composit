//! Agent-spec scanner — surfaces files that govern what AI agents are
//! allowed to do inside a repo. These are direct governance artifacts:
//! changing them changes agent behaviour, often without any code change.
//!
//! v0.1 covers three filename conventions in the wild:
//! - `SKILL.md` — Anthropic / Claude skill manifest. YAML frontmatter
//!   with `name`, `description`, sometimes `allowed-tools` and metadata.
//!   Scanner extracts the frontmatter; detection requires both a `name:`
//!   field and a body separator.
//! - `AGENTS.md` — Conventional repo-level agent instructions (Codex,
//!   community pattern). Free-form markdown; scanner records its
//!   existence and a one-line summary heuristic.
//! - `CLAUDE.md` — Claude Code project instructions. Same shape as
//!   AGENTS.md.
//!
//! All three are recorded as `agent_spec` resources with a `kind` extra
//! field distinguishing them. We do not parse the markdown body — the
//! governance signal is "does this file exist and what does it claim to
//! constrain". A future iteration could extract permission lists or
//! tool allowlists.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct AgentSpecScanner;

#[async_trait]
impl Scanner for AgentSpecScanner {
    fn id(&self) -> &str {
        "agent_spec"
    }

    fn name(&self) -> &str {
        "Agent Spec Scanner"
    }

    fn description(&self) -> &str {
        "Detects SKILL.md, AGENTS.md, CLAUDE.md — files that govern AI agent behaviour"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        for filename in ["SKILL.md", "AGENTS.md", "CLAUDE.md"] {
            // Look at every depth — skill repos nest SKILL.md per skill
            // (e.g. nuts-skills/skills/<name>/SKILL.md), and AGENTS.md /
            // CLAUDE.md sometimes appear in subprojects.
            let pattern = context.dir.join(format!("**/{filename}"));
            for entry in glob(&pattern.to_string_lossy())?.flatten() {
                if context.is_excluded(&entry) || !entry.is_file() {
                    continue;
                }
                if let Some(r) = parse_spec(&entry, &context.dir, filename) {
                    resources.push(r);
                }
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
            resolution: None,
        })
    }
}

fn parse_spec(path: &Path, base_dir: &Path, filename: &str) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;
    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let kind = match filename {
        "SKILL.md" => "skill",
        "AGENTS.md" => "agents",
        "CLAUDE.md" => "claude",
        _ => "unknown",
    };

    let frontmatter = extract_frontmatter(&content);

    // SKILL.md without parseable frontmatter is suspicious — likely a
    // documentation file someone happened to name SKILL.md. Skip it
    // unless the body contains a top-level "skill" header as a fallback.
    if filename == "SKILL.md" && frontmatter.is_none() && !looks_like_skill_doc(&content) {
        return None;
    }

    let mut extra = HashMap::new();
    extra.insert(
        "kind".to_string(),
        serde_json::Value::String(kind.to_string()),
    );

    let name = frontmatter.as_ref().and_then(|fm| {
        fm.get("name")
            .map(|v| v.trim().trim_matches('"').to_string())
    });

    if let Some(fm) = &frontmatter {
        if let Some(desc) = fm.get("description") {
            extra.insert(
                "description".to_string(),
                serde_json::Value::String(desc.trim().to_string()),
            );
        }
        if let Some(tools) = fm.get("allowed-tools") {
            extra.insert(
                "allowed_tools".to_string(),
                serde_json::Value::String(tools.trim().to_string()),
            );
        }
        if let Some(model) = fm.get("model") {
            extra.insert(
                "model".to_string(),
                serde_json::Value::String(model.trim().to_string()),
            );
        }
        if let Some(version) = fm.get("version") {
            extra.insert(
                "version".to_string(),
                serde_json::Value::String(version.trim().trim_matches('"').to_string()),
            );
        }
    }

    let line_count = content.lines().count();
    extra.insert(
        "lines".to_string(),
        serde_json::Value::Number(serde_json::Number::from(line_count)),
    );

    Some(Resource {
        resource_type: "agent_spec".to_string(),
        // Free-form AGENTS.md / CLAUDE.md have no name field — fall back
        // to the directory containing the file so multi-spec repos
        // (skills/<name>/SKILL.md) stay distinguishable in the report.
        name: name.or_else(|| {
            path.parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
        }),
        path: Some(format!("./{rel}")),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "agent_spec".to_string(),
        estimated_cost: None,
        extra,
    })
}

/// Parse a YAML-ish frontmatter block (`---\n…\n---`) into key/value
/// pairs. Robust against folded scalars (`description: >`) by collapsing
/// continuation lines into the previous key. Not a full YAML parser —
/// good enough for the v0.1 fields composit cares about (`name`,
/// `description`, `allowed-tools`, `model`, `version`).
fn extract_frontmatter(content: &str) -> Option<HashMap<String, String>> {
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }

    let mut block = String::new();
    let mut closed = false;
    for line in lines {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        block.push_str(line);
        block.push('\n');
    }
    if !closed {
        return None;
    }

    let mut map = HashMap::new();
    let mut current_key: Option<String> = None;
    let mut current_val = String::new();

    for raw in block.lines() {
        let trimmed = raw.trim_end();
        // New top-level key: starts at col 0, has a colon.
        if !raw.starts_with(' ') && !raw.starts_with('\t') {
            if let Some((k, v)) = trimmed.split_once(':') {
                if let Some(prev) = current_key.take() {
                    map.insert(prev, current_val.trim().to_string());
                    current_val.clear();
                }
                let key = k.trim().to_string();
                let initial = v.trim().trim_start_matches('>').trim().to_string();
                current_key = Some(key);
                current_val = initial;
                continue;
            }
        }
        // Continuation line for the current key.
        if current_key.is_some() {
            if !current_val.is_empty() {
                current_val.push(' ');
            }
            current_val.push_str(trimmed.trim());
        }
    }
    if let Some(k) = current_key.take() {
        map.insert(k, current_val.trim().to_string());
    }

    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

/// Fallback: a SKILL.md without YAML frontmatter is still a skill doc
/// if the first non-empty line is a top-level heading containing
/// "skill" (case-insensitive). Conservative — false negatives preferred
/// over false positives.
fn looks_like_skill_doc(content: &str) -> bool {
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        return t.starts_with('#') && t.to_lowercase().contains("skill");
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parses_skill_frontmatter_name_and_description() {
        let src = r#"---
name: my-skill
description: >
  Does something useful in a multi-line
  folded description.
allowed-tools: [WebSearch, Read]
---

# My Skill

Body text.
"#;
        let dir = tempdir().unwrap();
        let p = dir.path().join("SKILL.md");
        fs::write(&p, src).unwrap();

        let r = parse_spec(&p, dir.path(), "SKILL.md").expect("parses");
        assert_eq!(r.resource_type, "agent_spec");
        assert_eq!(r.name.as_deref(), Some("my-skill"));
        assert_eq!(r.extra.get("kind").and_then(|v| v.as_str()), Some("skill"));
        let desc = r.extra.get("description").and_then(|v| v.as_str()).unwrap();
        assert!(
            desc.contains("multi-line") && desc.contains("folded"),
            "folded description should be flattened: got {desc}"
        );
        assert!(r
            .extra
            .get("allowed_tools")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains("WebSearch")));
    }

    #[test]
    fn skill_without_frontmatter_falls_back_to_heading() {
        let src = "# Skill: do thing\n\nbody\n";
        let dir = tempdir().unwrap();
        let p = dir.path().join("SKILL.md");
        fs::write(&p, src).unwrap();

        let r = parse_spec(&p, dir.path(), "SKILL.md").expect("heading fallback");
        // name falls back to directory basename
        assert_eq!(
            r.name.as_deref(),
            dir.path().file_name().and_then(|n| n.to_str())
        );
    }

    #[test]
    fn skill_without_frontmatter_or_heading_is_skipped() {
        let src = "Just some random documentation.\n";
        let dir = tempdir().unwrap();
        let p = dir.path().join("SKILL.md");
        fs::write(&p, src).unwrap();

        assert!(parse_spec(&p, dir.path(), "SKILL.md").is_none());
    }

    #[test]
    fn agents_md_without_frontmatter_is_recorded() {
        let src = "# AGENTS.md\n\nScope: applies to the entire repository.\n";
        let dir = tempdir().unwrap();
        let p = dir.path().join("AGENTS.md");
        fs::write(&p, src).unwrap();

        let r = parse_spec(&p, dir.path(), "AGENTS.md").expect("free-form is OK for AGENTS.md");
        assert_eq!(r.extra.get("kind").and_then(|v| v.as_str()), Some("agents"));
        // No frontmatter → name falls back to directory basename
        assert_eq!(
            r.name.as_deref(),
            dir.path().file_name().and_then(|n| n.to_str())
        );
    }

    #[test]
    fn claude_md_recorded_with_kind() {
        let src = "# Claude Code instructions\n\nDo X.\n";
        let dir = tempdir().unwrap();
        let p = dir.path().join("CLAUDE.md");
        fs::write(&p, src).unwrap();

        let r = parse_spec(&p, dir.path(), "CLAUDE.md").expect("claude is recorded");
        assert_eq!(r.extra.get("kind").and_then(|v| v.as_str()), Some("claude"));
    }

    #[test]
    fn frontmatter_unterminated_returns_none() {
        let src = "---\nname: x\n(no closing dashes)\n";
        assert!(extract_frontmatter(src).is_none());
    }

    #[test]
    fn frontmatter_handles_quoted_strings() {
        let src = "---\nname: hookaido\nversion: \"2.6.0\"\n---\n";
        let fm = extract_frontmatter(src).unwrap();
        assert_eq!(fm.get("name").map(|s| s.as_str()), Some("hookaido"));
        assert_eq!(fm.get("version").map(|s| s.as_str()), Some("\"2.6.0\""));
    }
}
