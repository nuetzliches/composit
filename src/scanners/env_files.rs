use std::collections::HashMap;
use std::fs;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct EnvFilesScanner;

#[async_trait]
impl Scanner for EnvFilesScanner {
    fn id(&self) -> &str {
        "env_files"
    }

    fn name(&self) -> &str {
        "Environment Files Scanner"
    }

    fn description(&self) -> &str {
        "Detects .env files and counts variables (does not read values)"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        for pattern in &[".env", ".env.*"] {
            let full_pattern = context.dir.join("**").join(pattern);
            for entry in glob(&full_pattern.to_string_lossy())? {
                if let Ok(path) = entry {
                    if context.is_excluded(&path) {
                        continue;
                    }
                    // Skip .env files inside node_modules, target, .git etc.
                    let path_str = path.to_string_lossy();
                    if path_str.contains("node_modules")
                        || path_str.contains("/target/")
                        || path_str.contains("/.git/")
                    {
                        continue;
                    }

                    let content = fs::read_to_string(&path).unwrap_or_default();
                    let var_count = count_env_vars(&content);

                    let rel_path = path
                        .strip_prefix(&context.dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();

                    let mut extra = HashMap::new();
                    extra.insert(
                        "variables".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(var_count)),
                    );

                    resources.push(Resource {
                        resource_type: "env_file".to_string(),
                        name: None,
                        path: Some(format!("./{}", rel_path)),
                        provider: None,
                        created: None,
                        created_by: None,
                        detected_by: "env_files".to_string(),
                        estimated_cost: None,
                        extra,
                    });
                }
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}

/// Count assignments in a .env file. Ignores blank lines and comments.
fn count_env_vars(content: &str) -> usize {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && trimmed.contains('=')
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_count_env_vars_ignores_comments_and_blanks() {
        let content = r#"
# leading comment
FOO=bar
BAZ=qux

# another comment
EMPTY=
BROKEN_LINE_NO_EQUALS
QUOTED="hello world"
"#;
        assert_eq!(count_env_vars(content), 4);
    }

    #[test]
    fn test_count_env_vars_empty_file() {
        assert_eq!(count_env_vars(""), 0);
        assert_eq!(count_env_vars("# only comments\n# still comments\n"), 0);
    }

    #[tokio::test]
    async fn test_scan_detects_env_files_and_skips_ignored_dirs() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".env"), "A=1\nB=2\n").unwrap();
        fs::write(dir.path().join(".env.production"), "PROD=1\n").unwrap();

        // Should be ignored
        fs::create_dir_all(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join("node_modules/.env"), "X=1\n").unwrap();

        let scanner = EnvFilesScanner;
        let ctx = ScanContext {
            dir: dir.path().to_path_buf(),
            providers: vec![],
            skip_providers: true,
            exclude_patterns: vec![],
        };
        let result = scanner.scan(&ctx).await.unwrap();
        assert_eq!(result.resources.len(), 2);
        for r in &result.resources {
            assert_eq!(r.resource_type, "env_file");
        }
    }
}
