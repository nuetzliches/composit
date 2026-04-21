use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::core::types::Resource;

/// Known agent patterns in git author names/emails
const AGENT_PATTERNS: &[(&str, &str)] = &[
    ("claude", "agent:claude"),
    ("cursor", "agent:cursor"),
    ("devin", "agent:devin"),
    ("copilot", "agent:copilot"),
    ("github-actions", "agent:github-actions"),
    ("dependabot", "agent:dependabot"),
    ("renovate", "agent:renovate"),
    ("noreply@anthropic.com", "agent:claude"),
    ("[bot]", "agent:bot"),
];

struct GitFileInfo {
    author: String,
    date: String,
    co_authors: Vec<String>,
}

/// Enrich resources with git-blame attribution.
/// For each resource with a path, runs git log to find who created the file.
/// Detects Co-Authored-By headers to identify agent-assisted commits.
pub fn enrich_attribution(resources: &mut [Resource], base_dir: &Path) {
    for resource in resources.iter_mut() {
        if resource.created_by.is_some() {
            continue; // Already attributed (e.g. from provider API)
        }

        let path = match &resource.path {
            Some(p) => p.clone(),
            None => continue,
        };

        // Strip leading "./" for git commands
        let rel_path = path.strip_prefix("./").unwrap_or(&path);
        let full_path = base_dir.join(rel_path);

        if !full_path.exists() {
            continue;
        }

        // Get the first commit that introduced this file
        if let Some(info) = git_file_info(base_dir, rel_path, true) {
            let (attribution, extra) = classify_commit(&info);
            resource.created_by = Some(attribution);
            if resource.created.is_none() {
                resource.created = Some(info.date);
            }
            for (key, value) in extra {
                resource.extra.entry(key).or_insert(value);
            }
        }

        // Get the most recent commit that modified this file
        if let Some(info) = git_file_info(base_dir, rel_path, false) {
            let (attribution, _) = classify_commit(&info);
            resource.extra.insert(
                "last_modified_by".to_string(),
                serde_json::Value::String(attribution),
            );
            resource.extra.insert(
                "last_modified".to_string(),
                serde_json::Value::String(info.date),
            );
        }
    }
}

/// Run git log to find who created or last modified a file.
/// If `first` is true, finds the commit that first added the file.
/// If `first` is false, finds the most recent commit that touched the file.
fn git_file_info(repo_dir: &Path, file_path: &str, first: bool) -> Option<GitFileInfo> {
    let mut args = vec![
        "log".to_string(),
        "--format=%an <%ae>%x00%aI%x00%b%x00".to_string(),
    ];
    if first {
        args.push("--diff-filter=A".to_string());
        args.push("--follow".to_string());
        args.push("--reverse".to_string());
    } else {
        args.push("-1".to_string());
    }
    args.push("--".to_string());
    args.push(file_path.to_string());

    let output = Command::new("git")
        .args(&args)
        .current_dir(repo_dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return None;
    }

    // Split by NUL; first 3 fields are author, date, body
    let fields: Vec<&str> = stdout.splitn(4, '\0').collect();
    if fields.len() < 3 {
        return None;
    }

    let author = fields[0].trim().to_string();
    let date = fields[1]
        .trim()
        .split('T')
        .next()
        .unwrap_or(fields[1].trim())
        .to_string();
    let body = fields[2].to_string();

    if author.is_empty() {
        return None;
    }

    // Extract Co-Authored-By lines
    let co_authors: Vec<String> = body
        .lines()
        .filter(|line| line.trim().to_lowercase().starts_with("co-authored-by:"))
        .map(|line| {
            line.trim()
                .split_once(':')
                .map(|x| x.1)
                .unwrap_or("")
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();

    Some(GitFileInfo {
        author,
        date,
        co_authors,
    })
}

/// Classify a commit: extract all authors, flag agent involvement.
/// Returns (attribution_string, extra_fields).
/// The attribution always reflects the actual git commit author.
/// Agent co-authors are flagged via `agent_assisted` and `assisted_by`.
fn classify_commit(info: &GitFileInfo) -> (String, HashMap<String, serde_json::Value>) {
    let mut extra = HashMap::new();

    let author_classification = classify_author(&info.author);

    // Classify all co-authors
    let co_author_labels: Vec<String> = info
        .co_authors
        .iter()
        .map(|ca| classify_author(ca))
        .collect();

    if !co_author_labels.is_empty() {
        extra.insert(
            "co_authors".to_string(),
            serde_json::Value::Array(
                co_author_labels
                    .iter()
                    .map(|l| serde_json::Value::String(l.clone()))
                    .collect(),
            ),
        );
    }

    // Check co-authors for agent involvement
    for (i, label) in co_author_labels.iter().enumerate() {
        if label.starts_with("agent:") {
            extra.insert("agent_assisted".to_string(), serde_json::Value::Bool(true));
            extra.insert(
                "assisted_by".to_string(),
                serde_json::Value::String(label.clone()),
            );
            // Also store the raw co-author string for the first agent match
            extra
                .entry("assisted_by_raw".to_string())
                .or_insert_with(|| serde_json::Value::String(info.co_authors[i].clone()));
            break; // Use first agent match for assisted_by
        }
    }

    // Attribution is always the actual commit author
    (author_classification, extra)
}

/// Classify a git author string as agent or human
fn classify_author(author: &str) -> String {
    let lower = author.to_lowercase();
    for (pattern, label) in AGENT_PATTERNS {
        if lower.contains(pattern) {
            return label.to_string();
        }
    }
    // Extract name for human attribution
    let name = author
        .split('<')
        .next()
        .unwrap_or(author)
        .trim()
        .to_lowercase()
        .replace(' ', "-");
    format!("human:{}", name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_human() {
        assert_eq!(
            classify_author("Sebastian Meier <seb@example.com>"),
            "human:sebastian-meier"
        );
    }

    #[test]
    fn test_classify_agents() {
        assert_eq!(
            classify_author("Claude <noreply@anthropic.com>"),
            "agent:claude"
        );
        assert_eq!(
            classify_author("dependabot[bot] <support@github.com>"),
            "agent:dependabot"
        );
    }

    #[test]
    fn test_co_authored_by_detection() {
        let info = GitFileInfo {
            author: "7schmiede <seb@example.com>".to_string(),
            date: "2026-04-13".to_string(),
            co_authors: vec!["Claude Opus 4.6 (1M context) <noreply@anthropic.com>".to_string()],
        };
        let (attribution, extra) = classify_commit(&info);
        // Human stays as created_by, agent is only flagged
        assert_eq!(attribution, "human:7schmiede");
        assert_eq!(
            extra.get("agent_assisted").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            extra.get("assisted_by").and_then(|v| v.as_str()),
            Some("agent:claude")
        );
        // Co-authors list is populated
        let co_authors = extra
            .get("co_authors")
            .and_then(|v| v.as_array())
            .expect("co_authors must be populated as an array");
        assert_eq!(co_authors.len(), 1);
        assert_eq!(co_authors[0].as_str(), Some("agent:claude"));
    }

    #[test]
    fn test_pure_human_commit() {
        let info = GitFileInfo {
            author: "Sebastian <seb@example.com>".to_string(),
            date: "2026-04-13".to_string(),
            co_authors: vec![],
        };
        let (attribution, extra) = classify_commit(&info);
        assert_eq!(attribution, "human:sebastian");
        assert!(extra.is_empty());
    }

    #[test]
    fn test_human_with_human_coauthor() {
        let info = GitFileInfo {
            author: "Sebastian <seb@example.com>".to_string(),
            date: "2026-04-13".to_string(),
            co_authors: vec!["Tobi <tobi@example.com>".to_string()],
        };
        let (attribution, extra) = classify_commit(&info);
        assert_eq!(attribution, "human:sebastian");
        // No agent_assisted flag
        assert!(!extra.contains_key("agent_assisted"));
        // But co_authors list is populated
        let co_authors = extra
            .get("co_authors")
            .and_then(|v| v.as_array())
            .expect("co_authors must be populated as an array");
        assert_eq!(co_authors.len(), 1);
        assert_eq!(co_authors[0].as_str(), Some("human:tobi"));
    }

    #[test]
    fn test_multiple_coauthors_mixed() {
        let info = GitFileInfo {
            author: "Sebastian <seb@example.com>".to_string(),
            date: "2026-04-13".to_string(),
            co_authors: vec![
                "Tobi <tobi@example.com>".to_string(),
                "Claude Opus 4.6 (1M context) <noreply@anthropic.com>".to_string(),
            ],
        };
        let (attribution, extra) = classify_commit(&info);
        assert_eq!(attribution, "human:sebastian");
        assert_eq!(
            extra.get("agent_assisted").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            extra.get("assisted_by").and_then(|v| v.as_str()),
            Some("agent:claude")
        );
        let co_authors = extra
            .get("co_authors")
            .and_then(|v| v.as_array())
            .expect("co_authors must be populated as an array");
        assert_eq!(co_authors.len(), 2);
    }
}
