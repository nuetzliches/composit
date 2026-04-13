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

/// Enrich resources with git-blame attribution.
/// For each resource with a path, runs git log to find who created the file.
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
        if let Some((author, date)) = git_file_creator(base_dir, rel_path) {
            resource.created_by = Some(classify_author(&author));
            if resource.created.is_none() {
                resource.created = Some(date);
            }
        }
    }
}

/// Run git log to find who first created a file
fn git_file_creator(repo_dir: &Path, file_path: &str) -> Option<(String, String)> {
    let output = Command::new("git")
        .args([
            "log",
            "--diff-filter=A",
            "--follow",
            "--format=%an <%ae>|%aI",
            "--reverse",
            "--",
            file_path,
        ])
        .current_dir(repo_dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?;
    let mut parts = first_line.splitn(2, '|');
    let author = parts.next()?.trim().to_string();
    let date = parts
        .next()
        .map(|d| {
            // Truncate to date only: "2026-04-13T10:00:00+02:00" -> "2026-04-13"
            d.trim()
                .split('T')
                .next()
                .unwrap_or(d.trim())
                .to_string()
        })
        .unwrap_or_default();

    if author.is_empty() {
        return None;
    }

    Some((author, date))
}

/// Classify a git author as agent or human
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
        assert_eq!(classify_author("Sebastian Meier <seb@example.com>"), "human:sebastian-meier");
        assert_eq!(classify_author("John Doe <john@company.com>"), "human:john-doe");
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
        assert_eq!(
            classify_author("github-actions[bot] <noreply@github.com>"),
            "agent:github-actions"
        );
    }
}
