use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::Client;

use crate::core::types::{Provider, ProviderStatus, Report};

/// Load the report from disk and display aggregated status
pub async fn run_status(dir: &Path, live: bool) -> Result<()> {
    let report_path = dir.join("composit-report.yaml");
    if !report_path.exists() {
        anyhow::bail!(
            "No composit-report.yaml found in {}. Run `composit scan` first.",
            dir.display()
        );
    }

    let content = std::fs::read_to_string(&report_path)
        .with_context(|| format!("Failed to read {}", report_path.display()))?;
    let mut report: Report =
        serde_yaml::from_str(&content).with_context(|| "Failed to parse report")?;

    // Live provider checks
    if live && !report.providers.is_empty() {
        check_providers_live(&mut report.providers).await?;
    }

    print_status(&report);

    Ok(())
}

async fn check_providers_live(providers: &mut [Provider]) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("Failed to build HTTP client for live provider checks")?;

    for provider in providers.iter_mut() {
        let url = format!(
            "{}/.well-known/composit.json",
            provider.endpoint.trim_end_matches('/')
        );
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                provider.status = ProviderStatus::Reachable;
            }
            _ => {
                provider.status = ProviderStatus::Unreachable;
            }
        }
    }
    Ok(())
}

fn print_status(report: &Report) {
    println!();
    println!("{}", "composit status".bold());
    println!(
        "{} {}",
        "Workspace:".dimmed(),
        report.workspace.bold()
    );
    println!(
        "{} {}",
        "Last scan:".dimmed(),
        report.generated
    );
    println!("{}", "=".repeat(60));

    // Resources by type
    let mut by_type: HashMap<&str, usize> = HashMap::new();
    for r in &report.resources {
        *by_type.entry(&r.resource_type).or_insert(0) += 1;
    }

    println!();
    println!("  {}", "Resources".bold());
    let mut types: Vec<_> = by_type.iter().collect();
    types.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    for (resource_type, count) in &types {
        println!("    {:30} {}", resource_type, count.to_string().cyan());
    }

    // Attribution breakdown
    println!();
    println!("  {}", "Attribution".bold());

    let mut by_author: HashMap<&str, usize> = HashMap::new();
    let mut untracked = 0usize;
    for r in &report.resources {
        match &r.created_by {
            Some(author) => *by_author.entry(author).or_insert(0) += 1,
            None => untracked += 1,
        }
    }

    let mut authors: Vec<_> = by_author.iter().collect();
    authors.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    for (author, count) in &authors {
        let display = if author.starts_with("agent:") {
            author.yellow().to_string()
        } else {
            author.to_string()
        };
        println!("    {:30} {}", display, count);
    }
    if untracked > 0 {
        println!(
            "    {:30} {}",
            "untracked".dimmed(),
            untracked
        );
    }

    // Providers
    if !report.providers.is_empty() {
        println!();
        println!("  {}", "Providers".bold());
        for p in &report.providers {
            let status = match p.status {
                ProviderStatus::Reachable => "reachable".green(),
                ProviderStatus::Unreachable => "unreachable".red(),
                ProviderStatus::Unknown => "unknown".yellow(),
            };
            let caps = if p.capabilities.is_empty() {
                String::new()
            } else {
                format!(" ({})", p.capabilities.join(", ")).dimmed().to_string()
            };
            println!("    {:30} {}{}", p.name, status, caps);
        }
    }

    // Cost
    if report.summary.estimated_monthly_cost != "0 EUR" {
        println!();
        println!(
            "  {} ~{}/month",
            "Estimated cost:".bold(),
            report.summary.estimated_monthly_cost.cyan()
        );
    }

    // Summary line
    println!();
    println!("{}", "-".repeat(60));
    println!(
        "  {} resources across {} providers",
        report.summary.total_resources.to_string().bold(),
        report.summary.providers
    );
    println!();
}
