use std::path::{Path, PathBuf};

use anyhow::Result;
use async_trait::async_trait;

use super::types::{Provider, Resource};

pub struct ScanContext {
    pub dir: PathBuf,
    /// Provider targets to contact during the network phase. Each entry
    /// carries the public-manifest URL plus optional trust/auth metadata
    /// derived from a Compositfile. See RFC 002.
    pub providers: Vec<ProviderTarget>,
    pub skip_providers: bool,
    /// Compiled exclusion globs, matched against paths relative to `dir`.
    /// Scanners that walk the filesystem should call `is_excluded` before
    /// reading a file so a single Compositfile `scan.exclude_paths`
    /// entry suppresses the match for every scanner at once.
    pub exclude_patterns: Vec<glob::Pattern>,
}

impl ScanContext {
    pub fn is_excluded(&self, path: &Path) -> bool {
        if self.exclude_patterns.is_empty() {
            return false;
        }
        let rel = path.strip_prefix(&self.dir).unwrap_or(path);
        let rel_str = rel.to_string_lossy();
        self.exclude_patterns.iter().any(|p| p.matches(&rel_str))
    }
}

/// Compile a user-supplied entry into a glob pattern. Bare paths like
/// "tests/fixtures" are expanded to "tests/fixtures/**" so the caller
/// doesn't have to remember the `**` suffix for the common dir case.
pub fn compile_exclude_patterns(entries: &[String]) -> Vec<glob::Pattern> {
    entries
        .iter()
        .filter_map(|raw| {
            let trimmed = raw.trim().trim_end_matches('/');
            if trimmed.is_empty() {
                return None;
            }
            let normalised = if trimmed.contains(['*', '?', '[']) {
                trimmed.to_string()
            } else {
                format!("{}/**", trimmed)
            };
            glob::Pattern::new(&normalised).ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(dir: &str, patterns: &[&str]) -> ScanContext {
        ScanContext {
            dir: PathBuf::from(dir),
            providers: vec![],
            skip_providers: true,
            exclude_patterns: compile_exclude_patterns(
                &patterns.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            ),
        }
    }

    #[test]
    fn bare_dir_entry_excludes_subtree() {
        // `tests/fixtures` must exclude every file beneath it without the
        // user having to write `tests/fixtures/**` explicitly.
        let c = ctx("/repo", &["tests/fixtures"]);
        assert!(c.is_excluded(Path::new("/repo/tests/fixtures/docker/docker-compose.yml")));
        assert!(c.is_excluded(Path::new("/repo/tests/fixtures/demo-drift/Compositfile")));
        assert!(!c.is_excluded(Path::new("/repo/src/main.rs")));
        assert!(!c.is_excluded(Path::new("/repo/tests/scanner_e2e.rs")));
    }

    #[test]
    fn glob_entry_is_used_verbatim() {
        // User-supplied globs with metachars shouldn't get `**` appended.
        let c = ctx("/repo", &["**/*.generated.yaml"]);
        assert!(c.is_excluded(Path::new("/repo/a/b/c.generated.yaml")));
        assert!(!c.is_excluded(Path::new("/repo/a/b/c.yaml")));
    }

    #[test]
    fn empty_patterns_excludes_nothing() {
        let c = ctx("/repo", &[]);
        assert!(!c.is_excluded(Path::new("/repo/tests/fixtures/any.yml")));
    }

    #[test]
    fn trailing_slash_is_normalised() {
        // `tests/fixtures/` with a trailing slash must behave the same as
        // without — paths come from both `.gitignore`-style habits.
        let c = ctx("/repo", &["tests/fixtures/"]);
        assert!(c.is_excluded(Path::new("/repo/tests/fixtures/x.yaml")));
    }

    #[test]
    fn path_outside_scan_dir_is_matched_as_is() {
        // strip_prefix fails → we match against the absolute path. Rare in
        // practice but guards against panics if a scanner hands us a
        // path from an unexpected root.
        let c = ctx("/repo", &["tests/fixtures"]);
        assert!(!c.is_excluded(Path::new("/elsewhere/tests/fixtures/x.yml")));
    }
}

/// A provider to scan, with optional RFC-002 contract hints.
#[derive(Debug, Clone)]
pub struct ProviderTarget {
    /// Public manifest URL — must resolve to `/.well-known/composit.json`.
    pub url: String,
    /// Trust level declared in the Compositfile: "public" or "contract".
    /// None when the target was discovered opportunistically (e.g. from
    /// an MCP config) and no governance is attached.
    pub trust: Option<String>,
    /// Auth method the Compositfile expects — currently only `"api-key"`.
    pub auth_type: Option<String>,
    /// Name of the environment variable that holds the credential value.
    /// The scanner reads this lazily at request time.
    pub auth_env: Option<String>,
}

impl ProviderTarget {
    /// Convenience constructor for URL-only targets (public-tier only,
    /// no governance attached).
    pub fn public_only(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            trust: None,
            auth_type: None,
            auth_env: None,
        }
    }
}

pub struct ScanResult {
    pub resources: Vec<Resource>,
    pub providers: Vec<Provider>,
}

#[async_trait]
#[allow(dead_code)]
pub trait Scanner: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn needs_network(&self) -> bool;

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult>;
}
