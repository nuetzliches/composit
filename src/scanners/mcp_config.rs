use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::{Provider, ProviderStatus, Resource};

pub struct McpConfigScanner;

#[async_trait]
impl Scanner for McpConfigScanner {
    fn id(&self) -> &str {
        "mcp_config"
    }

    fn name(&self) -> &str {
        "MCP Config Scanner"
    }

    fn description(&self) -> &str {
        "Reads MCP server configurations (Claude Desktop, Cursor)"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();
        let mut providers = Vec::new();

        // Check Claude Desktop config
        let claude_config_paths = vec![
            dirs_config_path("claude", "claude_desktop_config.json"),
            dirs_config_path("Claude", "claude_desktop_config.json"),
        ];

        for config_path in claude_config_paths.into_iter().flatten() {
            if config_path.exists() {
                // Claude Desktop's config lives under $XDG_CONFIG_HOME, which
                // is outside the scan dir — we surface just the basename so
                // the report doesn't leak $HOME paths into diffs, dashboards,
                // or asciinema recordings. The `source: claude_desktop` in
                // extra keeps the origin clear.
                if let Ok((r, p)) = parse_mcp_config(&config_path, "claude_desktop", None) {
                    resources.extend(r);
                    providers.extend(p);
                }
            }
        }

        // Check Cursor config in project dir
        let cursor_config = context.dir.join(".cursor").join("mcp.json");
        if cursor_config.exists() {
            if let Ok((r, p)) = parse_mcp_config(&cursor_config, "cursor", Some(&context.dir)) {
                resources.extend(r);
                providers.extend(p);
            }
        }

        Ok(ScanResult {
            resources,
            providers,
        })
    }
}

fn dirs_config_path(app: &str, file: &str) -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join(app).join(file))
}

fn parse_mcp_config(
    path: &PathBuf,
    source: &str,
    base_dir: Option<&std::path::Path>,
) -> Result<(Vec<Resource>, Vec<Provider>)> {
    let content = std::fs::read_to_string(path)?;
    let config: serde_json::Value = serde_json::from_str(&content)?;

    // Render the path relative to the scan dir where possible so the report
    // stays portable (no $HOME leaks, diff-friendly). Out-of-tree configs
    // (Claude Desktop in $XDG_CONFIG_HOME) collapse to their basename; the
    // `source` field in extra preserves the origin.
    let display_path = match base_dir.and_then(|b| path.strip_prefix(b).ok()) {
        Some(rel) => format!("./{}", rel.display()),
        None => path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string()),
    };

    let mut resources = Vec::new();
    let mut providers = Vec::new();

    // Parse mcpServers section
    if let Some(servers) = config.get("mcpServers").and_then(|s| s.as_object()) {
        for (name, server_config) in servers {
            let mut extra = HashMap::new();
            extra.insert(
                "source".to_string(),
                serde_json::Value::String(source.to_string()),
            );

            if let Some(cmd) = server_config.get("command").and_then(|c| c.as_str()) {
                extra.insert(
                    "command".to_string(),
                    serde_json::Value::String(cmd.to_string()),
                );
            }

            if let Some(args) = server_config.get("args") {
                extra.insert("args".to_string(), args.clone());
            }

            resources.push(Resource {
                resource_type: "mcp_server".to_string(),
                name: Some(name.clone()),
                path: Some(display_path.clone()),
                provider: None,
                created: None,
                created_by: None,
                detected_by: "mcp_config".to_string(),
                estimated_cost: None,
                extra,
            });

            // If server has a URL/endpoint, add as provider for Phase 2
            if let Some(url) = server_config
                .get("url")
                .or_else(|| server_config.get("endpoint"))
                .and_then(|u| u.as_str())
            {
                providers.push(Provider {
                    name: name.clone(),
                    endpoint: url.to_string(),
                    protocol: "mcp".to_string(),
                    capabilities: vec![],
                    status: ProviderStatus::Unknown,
                    auth_mode: None,
                    auth_error: None,
                    contract: None,
                });
            }
        }
    }

    Ok((resources, providers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_mcp_config_extracts_servers_and_providers() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("mcp.json");
        let config_json = r#"{
            "mcpServers": {
                "stdio-server": {
                    "command": "npx",
                    "args": ["-y", "@acme/mcp"]
                },
                "remote-server": {
                    "url": "https://example.com/mcp"
                }
            }
        }"#;
        std::fs::write(&config_path, config_json).unwrap();

        let (resources, providers) =
            parse_mcp_config(&config_path, "cursor", Some(dir.path())).unwrap();
        assert_eq!(resources.len(), 2);

        // Resource-level assertions
        let stdio = resources
            .iter()
            .find(|r| r.name.as_deref() == Some("stdio-server"))
            .unwrap();
        assert_eq!(stdio.resource_type, "mcp_server");
        assert_eq!(
            stdio.extra.get("command").and_then(|v| v.as_str()),
            Some("npx")
        );
        assert_eq!(
            stdio.extra.get("source").and_then(|v| v.as_str()),
            Some("cursor")
        );
        assert!(stdio.extra.contains_key("args"));

        // Only the URL-based server should appear as a provider
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "remote-server");
        assert_eq!(providers[0].endpoint, "https://example.com/mcp");
        assert_eq!(providers[0].protocol, "mcp");
    }

    #[test]
    fn test_parse_mcp_config_missing_servers_section() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("mcp.json");
        std::fs::write(&config_path, r#"{"other": {}}"#).unwrap();

        let (resources, providers) =
            parse_mcp_config(&config_path, "claude_desktop", None).unwrap();
        assert!(resources.is_empty());
        assert!(providers.is_empty());
    }

    #[test]
    fn test_parse_mcp_config_invalid_json_errors() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("mcp.json");
        std::fs::write(&config_path, "not json").unwrap();

        let result = parse_mcp_config(&config_path, "cursor", Some(dir.path()));
        assert!(result.is_err());
    }
}
