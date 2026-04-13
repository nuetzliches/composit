mod cli;
mod core;
mod output;
mod scanners;

use std::fs;
use std::path::Path;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;

use cli::{Cli, Commands, OutputFormat};
use core::config::ScanConfig;
use core::registry::ScannerRegistry;
use core::scanner::ScanContext;
use core::report::{dedup_providers, dedup_resources};
use core::types::Report;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            dir,
            output,
            providers,
            no_providers,
            config,
            quiet,
        } => {
            let dir = fs::canonicalize(&dir)?;
            run_scan(&dir, output, providers, no_providers, config.as_deref(), quiet).await?;
        }
    }

    Ok(())
}

async fn run_scan(
    dir: &Path,
    format: OutputFormat,
    providers: Vec<String>,
    no_providers: bool,
    config_path: Option<&Path>,
    quiet: bool,
) -> Result<()> {
    // Load config
    let config = ScanConfig::load(dir, config_path)?;

    let mut registry = ScannerRegistry::new();
    scanners::register_default_scanners(&mut registry);

    // Register extra_patterns scanner if config has patterns
    if let Some(cfg) = &config {
        if !cfg.extra_patterns.is_empty() {
            registry.register(Box::new(
                scanners::extra_patterns::ExtraPatternsScanner {
                    patterns: cfg.extra_patterns.clone(),
                },
            ));
        }
    }

    // Merge CLI providers with config providers
    let mut all_providers = providers;
    if let Some(cfg) = &config {
        for entry in &cfg.providers {
            if !all_providers.contains(&entry.url) {
                all_providers.push(entry.url.clone());
            }
        }
    }

    let context = ScanContext {
        dir: dir.to_path_buf(),
        providers: all_providers,
        skip_providers: no_providers,
    };

    let result = registry.run_all(&context, config.as_ref()).await?;

    // Workspace name: config > directory name
    let workspace = config
        .as_ref()
        .and_then(|c| c.workspace.clone())
        .unwrap_or_else(|| {
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        });

    let providers = dedup_providers(result.providers);
    let mut resources = dedup_resources(result.resources);

    // Enrich with git-blame attribution
    core::attribution::enrich_attribution(&mut resources, dir);

    let report = Report::build(workspace, providers, resources);

    // Write report file
    let (content, filename) = match format {
        OutputFormat::Yaml => (output::yaml::to_yaml(&report)?, "composit-report.yaml"),
        OutputFormat::Json => (output::json::to_json(&report)?, "composit-report.json"),
    };

    let report_path = dir.join(filename);
    fs::write(&report_path, &content)?;

    if !quiet {
        output::terminal::print_summary(&report);
        println!(
            "  {} {}",
            "Report written to:".dimmed(),
            report_path.display()
        );
        println!();
    }

    Ok(())
}
