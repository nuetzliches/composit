use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ScanConfig {
    pub workspace: Option<String>,

    #[serde(default)]
    pub providers: Vec<ProviderEntry>,

    #[serde(default)]
    pub extra_patterns: Vec<ExtraPattern>,

    #[serde(default)]
    pub scanners: HashMap<String, bool>,

    /// Paths (relative to the scan dir) to skip during filesystem walks.
    /// Entries without glob metacharacters are treated as directory
    /// subtrees — "tests/fixtures" expands to "tests/fixtures/**".
    /// Patterns like "**/*.generated.yaml" are used verbatim.
    #[serde(default)]
    pub exclude_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderEntry {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtraPattern {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub glob: String,
    #[serde(default)]
    pub description: Option<String>,
}

impl ScanConfig {
    pub fn load(dir: &Path, explicit_path: Option<&Path>) -> Result<Option<ScanConfig>> {
        let config_path = if let Some(p) = explicit_path {
            p.to_path_buf()
        } else {
            let default_path = dir.join("composit.config.yaml");
            if !default_path.exists() {
                return Ok(None);
            }
            default_path
        };

        if !config_path.exists() {
            anyhow::bail!("Config file not found: {}", config_path.display());
        }

        let content = std::fs::read_to_string(&config_path)?;
        let config: ScanConfig = serde_yaml::from_str(&content)?;
        Ok(Some(config))
    }

    pub fn is_scanner_enabled(&self, scanner_id: &str) -> bool {
        self.scanners.get(scanner_id).copied().unwrap_or(true)
    }
}
