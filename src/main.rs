mod cli;
mod commands;
mod core;
mod output;
mod scanners;

use std::fs;
use std::path::Path;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;

use cli::{Cli, Commands, OutputFormat};
use core::compositfile::parse_compositfile;
use core::config::ScanConfig;
use core::registry::ScannerRegistry;
use core::report::{dedup_providers, dedup_resources};
use core::scanner::{compile_exclude_patterns, ProviderTarget, ScanContext};
use core::types::{Report, ScanMode};

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
            run_scan(
                &dir,
                output,
                providers,
                no_providers,
                config.as_deref(),
                quiet,
            )
            .await?;
        }
        Commands::Status { dir, live } => {
            let dir = fs::canonicalize(&dir)?;
            commands::status::run_status(&dir, live).await?;
        }
        Commands::Diff {
            dir,
            compositfile,
            report,
            output,
            strict,
            offline,
        } => {
            let dir = fs::canonicalize(&dir)?;
            let exit_code = commands::diff::run_diff(
                &dir,
                compositfile.as_deref(),
                report.as_deref(),
                output,
                strict,
                offline,
            )?;
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
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
            registry.register(Box::new(scanners::extra_patterns::ExtraPatternsScanner {
                patterns: cfg.extra_patterns.clone(),
            }));
        }
    }

    // Build provider targets: Compositfile (if present, governs trust/auth)
    // wins over CLI --providers, which wins over composit.config.yaml.
    // URLs deduplicate across sources by first-seen.
    let mut targets: Vec<ProviderTarget> = Vec::new();
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 1. Compositfile — carries trust + auth metadata for RFC 002 contract flows.
    let compositfile_path = dir.join("Compositfile");
    if compositfile_path.exists() {
        match parse_compositfile(&compositfile_path) {
            Ok(governance) => {
                for rule in &governance.providers {
                    if !seen_urls.insert(rule.manifest.clone()) {
                        continue;
                    }
                    let (auth_type, auth_env) = match &rule.auth {
                        Some(a) => (Some(a.auth_type.clone()), a.env.clone()),
                        None => (None, None),
                    };
                    targets.push(ProviderTarget {
                        url: rule.manifest.clone(),
                        trust: Some(rule.trust.clone()),
                        auth_type,
                        auth_env,
                    });
                }
            }
            Err(e) => {
                // Don't abort the scan if the Compositfile is broken —
                // the user can still want an inventory. Surface loudly.
                eprintln!(
                    "warning: Compositfile present at {} but could not be parsed: {}",
                    compositfile_path.display(),
                    e
                );
            }
        }
    }

    // 2. CLI --providers: public-only, no governance attached.
    for url in providers {
        if seen_urls.insert(url.clone()) {
            targets.push(ProviderTarget::public_only(url));
        }
    }

    // 3. composit.config.yaml providers: public-only fallback.
    if let Some(cfg) = &config {
        for entry in &cfg.providers {
            if seen_urls.insert(entry.url.clone()) {
                targets.push(ProviderTarget::public_only(entry.url.clone()));
            }
        }
    }

    // "offline" = we never attempted to contact provider manifests —
    // either --no-providers was passed, or no provider URLs are
    // configured.  Downstream (`composit diff`) uses this to downgrade
    // warnings that only make sense when providers were actually checked.
    let scan_mode = if no_providers || targets.is_empty() {
        ScanMode::Offline
    } else {
        ScanMode::Online
    };

    let exclude_patterns = config
        .as_ref()
        .map(|c| compile_exclude_patterns(&c.exclude_paths))
        .unwrap_or_default();

    let context = ScanContext {
        dir: dir.to_path_buf(),
        providers: targets,
        skip_providers: no_providers,
        exclude_patterns,
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

    let report = Report::build(workspace, providers, resources, scan_mode);

    // Write report file
    let (content, filename) = match format {
        OutputFormat::Yaml => (output::yaml::to_yaml(&report)?, "composit-report.yaml"),
        OutputFormat::Json => (output::json::to_json(&report)?, "composit-report.json"),
        OutputFormat::Html => (output::html::to_html(&report)?, "composit-report.html"),
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
