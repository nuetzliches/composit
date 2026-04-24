use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use tokio::process::Command;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct CronScanner;

#[async_trait]
impl Scanner for CronScanner {
    fn id(&self) -> &str {
        "cron"
    }

    fn name(&self) -> &str {
        "Cron Scanner"
    }

    fn description(&self) -> &str {
        "Reads crontab entries for the current user"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, _context: &ScanContext) -> Result<ScanResult> {
        let output = Command::new("crontab").arg("-l").output().await;

        let output = match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => {
                return Ok(ScanResult {
                    resources: vec![],
                    providers: vec![],
                    resolution: None,
                })
            }
        };

        Ok(ScanResult {
            resources: parse_crontab(&output),
            providers: vec![],
            resolution: None,
        })
    }
}

/// Parse `crontab -l` output into cron_job resources.
/// Skips blank lines and comments. Requires 5 time fields + command.
fn parse_crontab(output: &str) -> Vec<Resource> {
    let mut resources = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = trimmed.splitn(6, char::is_whitespace).collect();
        if parts.len() < 6 {
            continue;
        }

        let schedule = parts[..5].join(" ");
        let command = parts[5].to_string();

        let mut extra = HashMap::new();
        extra.insert("schedule".to_string(), serde_json::Value::String(schedule));
        extra.insert(
            "command".to_string(),
            serde_json::Value::String(command.clone()),
        );

        // Derive a name from the command (first word/binary)
        let name = command
            .split_whitespace()
            .next()
            .unwrap_or("unknown")
            .rsplit('/')
            .next()
            .unwrap_or("unknown")
            .to_string();

        resources.push(Resource {
            resource_type: "cron_job".to_string(),
            name: Some(name),
            path: None,
            provider: None,
            created: None,
            created_by: None,
            detected_by: "cron".to_string(),
            estimated_cost: None,
            extra,
        });
    }

    resources
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_crontab_basic_entries() {
        let output = r#"
# DO NOT EDIT — managed elsewhere
0 9 * * 1-5 /usr/local/bin/daily-report.sh
*/15 * * * * /opt/backup/run --quiet
"#;
        let resources = parse_crontab(output);
        assert_eq!(resources.len(), 2);

        let first = &resources[0];
        assert_eq!(first.resource_type, "cron_job");
        assert_eq!(first.name.as_deref(), Some("daily-report.sh"));
        assert_eq!(
            first.extra.get("schedule").and_then(|v| v.as_str()),
            Some("0 9 * * 1-5")
        );
        assert_eq!(
            first.extra.get("command").and_then(|v| v.as_str()),
            Some("/usr/local/bin/daily-report.sh")
        );

        let second = &resources[1];
        assert_eq!(second.name.as_deref(), Some("run"));
        assert_eq!(
            second.extra.get("schedule").and_then(|v| v.as_str()),
            Some("*/15 * * * *")
        );
    }

    #[test]
    fn test_parse_crontab_ignores_malformed_lines() {
        let output = "only three fields here\n\n# just a comment\n";
        let resources = parse_crontab(output);
        assert!(resources.is_empty());
    }

    #[test]
    fn test_parse_crontab_empty_input() {
        assert!(parse_crontab("").is_empty());
    }
}
