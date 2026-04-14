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

                // Show last_modified_by if different from created_by
                let last_mod = r
                    .extra
                    .get("last_modified_by")
                    .and_then(|v| v.as_str());
                let last_mod_str = match last_mod {
                    Some(lm) if Some(lm) != r.created_by.as_deref() => {
                        if lm.starts_with("agent:") {
                            format!(" -> {}", lm.yellow())
                        } else {
                            format!(" -> {}", lm.dimmed())
                        }
                    }
                    _ => String::new(),
                };

                // Show extra info inline
                let extra_info = format_extra(r);
                let extra_str = if extra_info.is_empty() {
                    String::new()
                } else {
                    format!(" {}", extra_info.dimmed())
                };

                println!(
                    "    {:<40} {:>20}{}  {}{}",
                    path, attribution, last_mod_str, date, extra_str
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

    // Count agent-modified (last_modified_by is agent, but created_by is not)
    let agent_modified = report
        .resources
        .iter()
        .filter(|r| {
            r.extra
                .get("last_modified_by")
                .and_then(|v| v.as_str())
                .map_or(false, |lm| {
                    lm.starts_with("agent:")
                        && !r
                            .created_by
                            .as_deref()
                            .map_or(false, |c| c.starts_with("agent:"))
                })
        })
        .count();

    let mut parts = vec![format!("{} resources", report.summary.total_resources)];
    if report.summary.agent_created > 0 {
        parts.push(format!(
            "{} agent-created",
            report.summary.agent_created.to_string().yellow()
        ));
    }
    if report.summary.agent_assisted > 0 {
        parts.push(format!(
            "{} agent-assisted",
            report.summary.agent_assisted.to_string().yellow()
        ));
    }
    if agent_modified > 0 {
        parts.push(format!(
            "{} agent-modified",
            agent_modified.to_string().yellow()
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
    if let Some(image) = r.extra.get("image").and_then(|v| v.as_str()) {
        parts.push(image.to_string());
    }
    if let Some(build) = r.extra.get("build").and_then(|v| v.as_str()) {
        parts.push(format!("build:{}", build));
    }
    if let Some(ports) = r.extra.get("ports").and_then(|v| v.as_array()) {
        let port_strs: Vec<&str> = ports.iter().filter_map(|p| p.as_str()).collect();
        if !port_strs.is_empty() {
            parts.push(format!("ports:{}", port_strs.join(",")));
        }
    }
    if let Some(networks) = r.extra.get("networks").and_then(|v| v.as_array()) {
        let net_strs: Vec<&str> = networks.iter().filter_map(|n| n.as_str()).collect();
        if !net_strs.is_empty() {
            parts.push(format!("nets:{}", net_strs.join(",")));
        }
    }
    if let Some(volumes) = r.extra.get("volumes").and_then(|v| v.as_array()) {
        let vol_strs: Vec<&str> = volumes.iter().filter_map(|v| v.as_str()).collect();
        if !vol_strs.is_empty() && r.resource_type == "docker_compose" {
            parts.push(format!("{} volumes", vol_strs.len()));
        }
    }
    if let Some(vars) = r.extra.get("variables").and_then(|v| v.as_u64()) {
        parts.push(format!("{} vars", vars));
    }
    if let Some(schedule) = r.extra.get("schedule").and_then(|v| v.as_str()) {
        parts.push(schedule.to_string());
    }
    // Terraform config summary
    if let Some(res) = r.extra.get("resources").and_then(|v| v.as_u64()) {
        if res > 0 {
            parts.push(format!("{} resources", res));
        }
    }
    if let Some(mods) = r.extra.get("modules").and_then(|v| v.as_u64()) {
        if mods > 0 {
            parts.push(format!("{} modules", mods));
        }
    }
    if let Some(providers) = r.extra.get("provider_list").and_then(|v| v.as_array()) {
        let prov_strs: Vec<&str> = providers.iter().filter_map(|p| p.as_str()).collect();
        if !prov_strs.is_empty() {
            parts.push(format!("providers:{}", prov_strs.join(",")));
        }
    }
    // Terraform resource type
    if let Some(rt) = r.extra.get("resource_type").and_then(|v| v.as_str()) {
        parts.push(rt.to_string());
    }
    // Terraform module source
    if let Some(source) = r.extra.get("source").and_then(|v| v.as_str()) {
        parts.push(source.to_string());
    }
    if let Some(version) = r.extra.get("version").and_then(|v| v.as_str()) {
        parts.push(format!("v{}", version));
    }
    if let Some(managed) = r.extra.get("managed_resources").and_then(|v| v.as_u64()) {
        if managed > 0 {
            parts.push(format!("{} managed", managed));
        }
    }
    // Caddyfile
    if let Some(sites) = r.extra.get("sites").and_then(|v| v.as_u64()) {
        parts.push(format!("{} sites", sites));
    }
    if let Some(rp) = r.extra.get("reverse_proxy").and_then(|v| v.as_str()) {
        parts.push(format!("-> {}", rp));
    }
    if r.extra.get("file_server").and_then(|v| v.as_bool()).unwrap_or(false) {
        parts.push("file_server".to_string());
    }
    // Workflows
    if let Some(platform) = r.extra.get("platform").and_then(|v| v.as_str()) {
        parts.push(platform.to_string());
    }
    if let Some(triggers) = r.extra.get("triggers").and_then(|v| v.as_array()) {
        let trig_strs: Vec<&str> = triggers.iter().filter_map(|t| t.as_str()).collect();
        if !trig_strs.is_empty() {
            parts.push(format!("on:{}", trig_strs.join(",")));
        }
    }
    if let Some(jobs) = r.extra.get("jobs").and_then(|v| v.as_u64()) {
        if jobs > 0 && r.resource_type == "workflow" {
            parts.push(format!("{} jobs", jobs));
        }
    }
    if let Some(runner) = r.extra.get("runs_on").and_then(|v| v.as_str()) {
        parts.push(format!("runner:{}", runner));
    }
    // Prometheus
    if let Some(sc) = r.extra.get("scrape_configs").and_then(|v| v.as_u64()) {
        parts.push(format!("{} scrape configs", sc));
    }
    if let Some(rules) = r.extra.get("rules").and_then(|v| v.as_u64()) {
        parts.push(format!("{} rules", rules));
    }
    if let Some(groups) = r.extra.get("groups").and_then(|v| v.as_u64()) {
        if groups > 0 && r.resource_type == "prometheus_rules" {
            parts.push(format!("{} groups", groups));
        }
    }
    if r.extra.get("alerting").and_then(|v| v.as_bool()).unwrap_or(false) {
        parts.push("alerting".to_string());
    }

    parts.join(", ")
}
