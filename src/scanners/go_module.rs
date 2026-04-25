//! Go module scanner — surfaces `go.mod` files.
//!
//! A `go.mod` declares the module path and Go version for a Go project.
//! Multi-module repos can host several. Composit treats each as a
//! `go_module` resource so role rules can scope by module path or pin
//! the toolchain version.
//!
//! Format reference: https://go.dev/ref/mod#go-mod-file
//!
//! Tracked fields:
//! - `module <path>` — first non-comment statement; becomes the resource name.
//! - `go <version>` — toolchain pin; recorded as the `go_version` extra.
//! - `require` count — direct + indirect, as a sanity-check signal that
//!   roles can constrain (e.g. "modules SHOULD have <100 dependencies").

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct GoModuleScanner;

#[async_trait]
impl Scanner for GoModuleScanner {
    fn id(&self) -> &str {
        "go_module"
    }

    fn name(&self) -> &str {
        "Go Module Scanner"
    }

    fn description(&self) -> &str {
        "Detects go.mod — module path, Go version, dependency count"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        let pattern = context.dir.join("**/go.mod");
        for entry in glob(&pattern.to_string_lossy())?.flatten() {
            if context.is_excluded(&entry) || !entry.is_file() {
                continue;
            }
            // Skip vendored dependencies.
            if entry.components().any(|c| c.as_os_str() == "vendor") {
                continue;
            }
            if let Some(r) = parse_go_mod(&entry, &context.dir) {
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

fn parse_go_mod(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;
    let parsed = parse_fields(&content)?;

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let mut extra = HashMap::new();
    if let Some(v) = &parsed.go_version {
        extra.insert(
            "go_version".to_string(),
            serde_json::Value::String(v.clone()),
        );
    }
    extra.insert(
        "direct_requires".to_string(),
        serde_json::Value::Number(serde_json::Number::from(parsed.direct_count)),
    );
    extra.insert(
        "indirect_requires".to_string(),
        serde_json::Value::Number(serde_json::Number::from(parsed.indirect_count)),
    );

    Some(Resource {
        resource_type: "go_module".to_string(),
        name: parsed.module.clone(),
        path: Some(format!("./{rel}")),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "go_module".to_string(),
        estimated_cost: None,
        extra,
    })
}

#[derive(Default, Debug)]
struct GoModFields {
    module: Option<String>,
    go_version: Option<String>,
    direct_count: usize,
    indirect_count: usize,
}

fn parse_fields(content: &str) -> Option<GoModFields> {
    let mut fields = GoModFields::default();
    let mut in_require_block = false;

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        if in_require_block {
            if line.starts_with(')') {
                in_require_block = false;
                continue;
            }
            count_require_line(line, &mut fields);
            continue;
        }

        if let Some(rest) = line.strip_prefix("module ") {
            fields.module = Some(rest.trim().trim_matches('"').to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("go ") {
            fields.go_version = Some(rest.trim().to_string());
            continue;
        }
        if line == "require (" || line.starts_with("require (") {
            in_require_block = true;
            continue;
        }
        if let Some(rest) = line.strip_prefix("require ") {
            // Inline single-line require: `require module/path v1.0.0`
            count_require_line(rest, &mut fields);
        }
    }

    // require module presence: at least a `module` line is needed to
    // call this a real go.mod (not a stray markdown snippet).
    fields.module.as_ref()?;
    Some(fields)
}

fn count_require_line(line: &str, fields: &mut GoModFields) {
    if line.is_empty() {
        return;
    }
    if line.contains("// indirect") {
        fields.indirect_count += 1;
    } else {
        fields.direct_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parses_module_and_go_version() {
        let src = r#"module example.com/foo

go 1.24.0
"#;
        let f = parse_fields(src).expect("module is present");
        assert_eq!(f.module.as_deref(), Some("example.com/foo"));
        assert_eq!(f.go_version.as_deref(), Some("1.24.0"));
        assert_eq!(f.direct_count, 0);
        assert_eq!(f.indirect_count, 0);
    }

    #[test]
    fn counts_direct_and_indirect_requires() {
        let src = r#"module example.com/foo

go 1.21

require (
    github.com/spf13/cobra v1.8.0
    github.com/stretchr/testify v1.9.0
    github.com/sirupsen/logrus v1.9.3 // indirect
    github.com/google/uuid v1.6.0 // indirect
    github.com/dustin/go-humanize v1.0.1 // indirect
)
"#;
        let f = parse_fields(src).expect("parses");
        assert_eq!(f.direct_count, 2);
        assert_eq!(f.indirect_count, 3);
    }

    #[test]
    fn inline_require_line() {
        let src = "module example.com/x\n\ngo 1.21\n\nrequire example.com/y v1.0.0\n";
        let f = parse_fields(src).expect("parses");
        assert_eq!(f.direct_count, 1);
        assert_eq!(f.indirect_count, 0);
    }

    #[test]
    fn no_module_line_returns_none() {
        let src = "go 1.21\n";
        assert!(parse_fields(src).is_none());
    }

    #[test]
    fn parse_go_mod_file_end_to_end() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("go.mod");
        fs::write(
            &p,
            "module example.com/svc\n\ngo 1.22\n\nrequire example.com/lib v1.0.0\n",
        )
        .unwrap();

        let r = parse_go_mod(&p, dir.path()).expect("parses");
        assert_eq!(r.resource_type, "go_module");
        assert_eq!(r.name.as_deref(), Some("example.com/svc"));
        assert_eq!(
            r.extra.get("go_version").and_then(|v| v.as_str()),
            Some("1.22")
        );
        assert_eq!(
            r.extra
                .get("direct_requires")
                .and_then(|v| v.as_u64()),
            Some(1)
        );
    }
}
