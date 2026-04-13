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
            _ => return Ok(ScanResult { resources: vec![], providers: vec![] }),
        };

        let mut resources = Vec::new();

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Cron lines have 5 time fields followed by the command
            let parts: Vec<&str> = trimmed.splitn(6, char::is_whitespace).collect();
            if parts.len() >= 6 {
                let schedule = parts[..5].join(" ");
                let command = parts[5].to_string();

                let mut extra = HashMap::new();
                extra.insert(
                    "schedule".to_string(),
                    serde_json::Value::String(schedule),
                );
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
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}
