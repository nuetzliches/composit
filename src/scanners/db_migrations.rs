use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct DbMigrationsScanner;

#[async_trait]
impl Scanner for DbMigrationsScanner {
    fn id(&self) -> &str {
        "db_migrations"
    }

    fn name(&self) -> &str {
        "DB Migrations Scanner"
    }

    fn description(&self) -> &str {
        "Detects migration sets — Alembic, sqlx, Flyway, Prisma; counts files"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        for detector in detectors() {
            if let Some(r) = detector.detect(&context.dir, &context.exclude_patterns) {
                resources.push(r);
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
            resolution: None,
        })
    }
}

struct MigrationDetector {
    framework: &'static str,
    glob_pattern: &'static str,
    // If set, each file must contain this string to count
    file_guard: Option<&'static str>,
}

impl MigrationDetector {
    fn detect(&self, dir: &Path, exclude: &[glob::Pattern]) -> Option<Resource> {
        let full_pattern = dir.join(self.glob_pattern);
        let mut count = 0usize;
        let mut migration_dir: Option<String> = None;

        for entry in glob(&full_pattern.to_string_lossy()).ok()?.flatten() {
            if !entry.is_file() {
                continue;
            }
            let rel = entry.strip_prefix(dir).unwrap_or(&entry);
            let rel_str = rel.to_string_lossy();
            if exclude.iter().any(|p| p.matches(&rel_str)) {
                continue;
            }
            if let Some(guard) = self.file_guard {
                let content = match std::fs::read_to_string(&entry) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                if !content.contains(guard) {
                    continue;
                }
            }
            count += 1;
            if migration_dir.is_none() {
                if let Some(parent) = entry.parent() {
                    if let Ok(rel) = parent.strip_prefix(dir) {
                        migration_dir = Some(format!("./{}", rel.to_string_lossy()));
                    }
                }
            }
        }

        if count == 0 {
            return None;
        }

        let mut extra = HashMap::new();
        extra.insert(
            "framework".to_string(),
            serde_json::Value::String(self.framework.to_string()),
        );
        extra.insert(
            "migration_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(count)),
        );

        Some(Resource {
            resource_type: "db_migrations".to_string(),
            name: Some(self.framework.to_string()),
            path: migration_dir,
            provider: None,
            created: None,
            created_by: None,
            detected_by: "db_migrations".to_string(),
            estimated_cost: None,
            extra,
        })
    }
}

fn detectors() -> Vec<MigrationDetector> {
    vec![
        MigrationDetector {
            framework: "alembic",
            // alembic creates a versions/ directory; each migration is a .py file
            // with a `def upgrade()` function.
            glob_pattern: "**/versions/*.py",
            file_guard: Some("def upgrade"),
        },
        MigrationDetector {
            framework: "sqlx",
            // sqlx migrations live in migrations/ as *.sql files with
            // a `-- Add migration script here` header.
            glob_pattern: "**/migrations/*.sql",
            file_guard: None,
        },
        MigrationDetector {
            framework: "flyway",
            // Flyway uses V<version>__<description>.sql naming under db/migration/
            glob_pattern: "**/db/migration/V*.sql",
            file_guard: None,
        },
        MigrationDetector {
            framework: "prisma",
            // Prisma generates prisma/migrations/<timestamp>_*/migration.sql
            glob_pattern: "**/prisma/migrations/*/migration.sql",
            file_guard: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn detects_alembic_migrations() {
        let tmp = tempdir().unwrap();
        let versions = tmp.path().join("versions");
        fs::create_dir_all(&versions).unwrap();
        fs::write(
            versions.join("001_add_users.py"),
            "def upgrade():\n    pass\ndef downgrade():\n    pass\n",
        )
        .unwrap();
        fs::write(
            versions.join("002_add_posts.py"),
            "def upgrade():\n    pass\n",
        )
        .unwrap();

        let d = MigrationDetector {
            framework: "alembic",
            glob_pattern: "**/versions/*.py",
            file_guard: Some("def upgrade"),
        };
        let r = d.detect(tmp.path(), &[]).unwrap();
        assert_eq!(r.name, Some("alembic".to_string()));
        assert_eq!(r.extra["migration_count"].as_u64(), Some(2));
    }

    #[test]
    fn detects_prisma_migrations() {
        let tmp = tempdir().unwrap();
        let m1 = tmp.path().join("prisma/migrations/20240101_init");
        fs::create_dir_all(&m1).unwrap();
        fs::write(m1.join("migration.sql"), "CREATE TABLE users (id INT);\n").unwrap();

        let d = MigrationDetector {
            framework: "prisma",
            glob_pattern: "**/prisma/migrations/*/migration.sql",
            file_guard: None,
        };
        let r = d.detect(tmp.path(), &[]).unwrap();
        assert_eq!(r.extra["migration_count"].as_u64(), Some(1));
    }

    #[test]
    fn returns_none_when_no_migrations_found() {
        let tmp = tempdir().unwrap();
        let d = MigrationDetector {
            framework: "flyway",
            glob_pattern: "**/db/migration/V*.sql",
            file_guard: None,
        };
        assert!(d.detect(tmp.path(), &[]).is_none());
    }
}
