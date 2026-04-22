use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct DeployScriptsScanner;

#[async_trait]
impl Scanner for DeployScriptsScanner {
    fn id(&self) -> &str {
        "deploy_scripts"
    }

    fn name(&self) -> &str {
        "Deploy Scripts Scanner"
    }

    fn description(&self) -> &str {
        "Detects shell deploy/bootstrap scripts under scripts/, deploy/, bin/"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let patterns = [
            "**/scripts/deploy*",
            "**/scripts/release*",
            "**/scripts/bootstrap*",
            "**/scripts/publish*",
            "**/deploy/*.sh",
            "**/deploy/*.bash",
            "**/bin/deploy*",
            "**/bin/release*",
            "**/Makefile",
        ];

        for pattern in &patterns {
            let full_pattern = context.dir.join(pattern);
            for entry in glob(&full_pattern.to_string_lossy())?.flatten() {
                if context.is_excluded(&entry) || !entry.is_file() {
                    continue;
                }
                if !seen.insert(entry.clone()) {
                    continue;
                }
                // Makefiles are common; only include if they contain deploy targets
                if entry.file_name().and_then(|n| n.to_str()) == Some("Makefile") {
                    if !has_deploy_target(&entry) {
                        continue;
                    }
                }
                if let Some(r) = build_resource(&entry, &context.dir) {
                    resources.push(r);
                }
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}

fn build_resource(path: &Path, base_dir: &Path) -> Option<Resource> {
    let file_name = path.file_name()?.to_string_lossy().to_string();
    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let kind = classify(&file_name);
    let mut extra = HashMap::new();
    extra.insert("kind".to_string(), serde_json::Value::String(kind));

    Some(Resource {
        resource_type: "deploy_script".to_string(),
        name: Some(file_name),
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "deploy_scripts".to_string(),
        estimated_cost: None,
        extra,
    })
}

fn classify(name: &str) -> String {
    if name == "Makefile" {
        return "makefile".to_string();
    }
    let lower = name.to_lowercase();
    if lower.contains("bootstrap") {
        "bootstrap".to_string()
    } else if lower.contains("release") || lower.contains("publish") {
        "release".to_string()
    } else {
        "deploy".to_string()
    }
}

fn has_deploy_target(path: &Path) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    // A Makefile deploy target starts at column 0 and ends with ':'
    content.lines().any(|l| {
        let l = l.trim_end();
        (l.starts_with("deploy") || l.starts_with("release") || l.starts_with("publish"))
            && l.ends_with(':')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_script_kinds() {
        assert_eq!(classify("deploy.sh"), "deploy".to_string());
        assert_eq!(classify("bootstrap.sh"), "bootstrap".to_string());
        assert_eq!(classify("release.sh"), "release".to_string());
        assert_eq!(classify("publish.sh"), "release".to_string());
        assert_eq!(classify("Makefile"), "makefile".to_string());
    }

    #[test]
    fn makefile_guard_requires_deploy_target() {
        use std::fs;
        use tempfile::tempdir;

        let tmp = tempdir().unwrap();
        let mf = tmp.path().join("Makefile");
        fs::write(&mf, "build:\n\tcargo build\n\ntest:\n\tcargo test\n").unwrap();
        assert!(!has_deploy_target(&mf));

        fs::write(&mf, "deploy:\n\tcargo build --release\n").unwrap();
        assert!(has_deploy_target(&mf));
    }
}
