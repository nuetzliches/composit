mod cli;
mod commands;
mod core;
mod output;
mod scanners;

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;

use cli::{Cli, Commands, OutputFormat};
use core::compositfile::parse_compositfile;
use core::governance::Governance;
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
            quiet,
        } => {
            let dir = fs::canonicalize(&dir)?;
            run_scan(&dir, output, providers, no_providers, quiet).await?;
        }
        Commands::Status { dir, live } => {
            let dir = fs::canonicalize(&dir)?;
            commands::status::run_status(&dir, live).await?;
        }
        Commands::Init {
            dir,
            workspace,
            minimal,
        } => {
            let dir = fs::canonicalize(&dir)?;

            // Check before the expensive scan — abort early if user declines overwrite.
            let compositfile_path = dir.join("Compositfile");
            if compositfile_path.exists() {
                print!(
                    "  {} Compositfile already exists. Overwrite? [y/N] ",
                    "!".yellow()
                );
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("  {}", "Aborted.".yellow());
                    return Ok(());
                }
            }

            let report = if !minimal {
                Some(build_report(&dir, vec![], false).await?)
            } else {
                None
            };

            // Persist the scan report alongside the Compositfile so the
            // advertised `composit init` → `composit diff` flow works
            // without an intermediate `composit scan` step.
            if let Some(r) = &report {
                fs::write(dir.join("composit-report.yaml"), output::yaml::to_yaml(r)?)?;
            }

            commands::init::run_init(&dir, workspace, report.as_ref())?;
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
    quiet: bool,
) -> Result<()> {
    let report = build_report(dir, providers, no_providers).await?;

    let (content, filename) = match format {
        OutputFormat::Yaml => (output::yaml::to_yaml(&report)?, "composit-report.yaml"),
        OutputFormat::Json => (output::json::to_json(&report)?, "composit-report.json"),
        OutputFormat::Html => (output::html::to_html(&report)?, "composit-report.html"),
    };

    let report_path = dir.join(filename);
    fs::write(&report_path, &content)?;

    if !quiet {
        output::terminal::print_summary(&report);
        // Show the path relative to CWD when possible — keeps the terminal
        // output short and, crucially, doesn't leak $HOME into asciinema
        // recordings or HN screenshots.
        let display_path = std::env::current_dir()
            .ok()
            .and_then(|cwd| report_path.strip_prefix(&cwd).ok().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| report_path.clone());
        println!(
            "  {} {}",
            "Report written to:".dimmed(),
            display_path.display()
        );
        println!();
    }

    Ok(())
}

/// Run all scanners and return the Report without writing anything to disk.
async fn build_report(dir: &Path, providers: Vec<String>, no_providers: bool) -> Result<Report> {
    // The Compositfile is the single source of truth for governance AND
    // scanner tuning (exclude_paths, extra_patterns, scanner toggles,
    // provider list). Missing file is fine — scanner falls back to
    // dirname-based workspace and no tuning.
    let governance = load_governance(dir);

    let mut registry = ScannerRegistry::new();
    scanners::register_default_scanners(&mut registry);

    // Register extra_patterns scanner if the Compositfile declared any.
    if let Some(gov) = &governance {
        if !gov.scan.extra_patterns.is_empty() {
            registry.register(Box::new(scanners::extra_patterns::ExtraPatternsScanner {
                patterns: gov.scan.extra_patterns.clone(),
            }));
        }
    }

    // Build provider targets: Compositfile providers carry full trust/auth
    // metadata; CLI --providers adds public-only overrides on top. URLs
    // dedupe across sources by first-seen.
    let mut targets: Vec<ProviderTarget> = Vec::new();
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();

    if let Some(gov) = &governance {
        for rule in &gov.providers {
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

    for url in providers {
        if seen_urls.insert(url.clone()) {
            targets.push(ProviderTarget::public_only(url));
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

    let exclude_patterns = governance
        .as_ref()
        .map(|g| compile_exclude_patterns(&g.scan.exclude_paths))
        .unwrap_or_default();

    let context = ScanContext {
        dir: dir.to_path_buf(),
        providers: targets,
        skip_providers: no_providers,
        exclude_patterns,
    };

    let scan_settings = governance.as_ref().map(|g| &g.scan);
    let result = registry.run_all(&context, scan_settings).await?;

    // Workspace name: Compositfile label > directory name.
    let workspace = governance
        .as_ref()
        .map(|g| g.workspace.clone())
        .unwrap_or_else(|| {
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        });

    let providers = dedup_providers(result.providers);
    let mut resources = dedup_resources(result.resources);

    // Enrich with git-blame attribution
    core::attribution::enrich_attribution(&mut resources, dir);

    let mut report = Report::build(workspace, providers, resources, scan_mode);
    report.resolution = result.resolution;
    Ok(report)
}

/// Load the Compositfile at the scan root. A missing file is fine —
/// governance is optional. A present-but-broken file is surfaced loudly
/// so the operator notices, but the scan continues so they still get an
/// inventory.
fn load_governance(dir: &Path) -> Option<Governance> {
    let path = dir.join("Compositfile");
    if !path.exists() {
        return None;
    }
    match parse_compositfile(&path) {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!(
                "warning: Compositfile present at {} but could not be parsed: {}",
                path.display(),
                e
            );
            None
        }
    }
}
