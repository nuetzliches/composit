use std::collections::HashMap;

use colored::Colorize;

use crate::core::types::Report;

pub fn print_summary(report: &Report) {
    println!();
    println!("{}", "composit scan".bold());
    println!("{}", "=".repeat(60));

    // Group resources by type
    let mut by_type: HashMap<&str, Vec<&crate::core::types::Resource>> = HashMap::new();
    for r in &report.resources {
        by_type.entry(&r.resource_type).or_default().push(r);
    }

    if report.resources.is_empty() {
        println!();
        println!("  {}", "No resources detected.".dimmed());
    } else {
        // Show resources grouped by type with details
        let mut types: Vec<_> = by_type.iter().collect();
        types.sort_by_key(|(t, _)| **t);

        for (resource_type, resources) in &types {
            println!();
            println!(
                "  {} {}",
                resource_type.bold(),
                format!("({})", resources.len()).dimmed()
            );
            for r in *resources {
                let path = r.path.as_deref().unwrap_or("");
                let attribution = match &r.created_by {
                    Some(a) if a.starts_with("agent:") => a.yellow().to_string(),
                    Some(a) if a.starts_with("human:") => a.dimmed().to_string(),
                    _ => "untracked".dimmed().to_string(),
                };
                let date = r
                    .created
                    .as_deref()
                    .unwrap_or("")
                    .dimmed()
                    .to_string();

                // Show extra info inline
                let extra_info = format_extra(r);
                let extra_str = if extra_info.is_empty() {
                    String::new()
                } else {
                    format!(" {}", extra_info.dimmed())
                };

                println!(
                    "    {:<40} {:>20}  {}{}",
                    path, attribution, date, extra_str
                );
            }
        }
    }

    // Providers
    if !report.providers.is_empty() {
        println!();
        println!("  {}", "Providers".bold());
        for p in &report.providers {
            let status = match p.status {
                crate::core::types::ProviderStatus::Reachable => "reachable".green(),
                crate::core::types::ProviderStatus::Unreachable => "unreachable".red(),
                crate::core::types::ProviderStatus::Unknown => "unknown".yellow(),
            };
            println!("    {:<40} {}", p.name, status);
        }
    }

    // Summary
    println!();
    println!("{}", "-".repeat(60));
    let mut parts = vec![format!("{} resources", report.summary.total_resources)];
    if report.summary.agent_created > 0 {
        parts.push(format!(
            "{} agent-created",
            report.summary.agent_created.to_string().yellow()
        ));
    }
    if report.summary.human_created > 0 {
        parts.push(format!("{} human-created", report.summary.human_created));
    }
    if report.summary.auto_detected > 0 {
        parts.push(format!("{} untracked", report.summary.auto_detected));
    }
    if report.summary.estimated_monthly_cost != "0 EUR" {
        parts.push(format!(
            "~{}/month",
            report.summary.estimated_monthly_cost.cyan()
        ));
    }
    println!("  {}", parts.join(" | "));
    println!();
}

fn format_extra(r: &crate::core::types::Resource) -> String {
    let mut parts = Vec::new();

    if let Some(services) = r.extra.get("services").and_then(|v| v.as_u64()) {
        parts.push(format!("{} services", services));
    }
    if let Some(vars) = r.extra.get("variables").and_then(|v| v.as_u64()) {
        parts.push(format!("{} vars", vars));
    }
    if let Some(schedule) = r.extra.get("schedule").and_then(|v| v.as_str()) {
        parts.push(schedule.to_string());
    }
    if let Some(managed) = r.extra.get("managed_resources").and_then(|v| v.as_u64()) {
        if managed > 0 {
            parts.push(format!("{} managed", managed));
        }
    }

    parts.join(", ")
}
