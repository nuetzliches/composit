use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "composit", version, about = "Governance-as-Code for AI-generated infrastructure")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Scan for agent-created infrastructure and generate an inventory report
    Scan {
        /// Directory to scan
        #[arg(long, default_value = ".")]
        dir: PathBuf,

        /// Output format
        #[arg(long, default_value = "yaml")]
        output: OutputFormat,

        /// Explicit provider URLs to connect to
        #[arg(long)]
        providers: Vec<String>,

        /// Skip provider API calls (filesystem scan only)
        #[arg(long)]
        no_providers: bool,

        /// Path to composit.config.yaml
        #[arg(long)]
        config: Option<PathBuf>,

        /// Only write report file, no terminal summary
        #[arg(long)]
        quiet: bool,
    },
    /// Show aggregated status from the last scan report
    Status {
        /// Directory containing composit-report.yaml
        #[arg(long, default_value = ".")]
        dir: PathBuf,

        /// Check live provider reachability
        #[arg(long)]
        live: bool,
    },
    /// Compare Compositfile governance against scan report
    Diff {
        /// Directory containing Compositfile and composit-report.yaml
        #[arg(long, default_value = ".")]
        dir: PathBuf,

        /// Path to Compositfile
        #[arg(long)]
        compositfile: Option<PathBuf>,

        /// Path to composit-report.yaml
        #[arg(long)]
        report: Option<PathBuf>,

        /// Output format
        #[arg(long, default_value = "terminal")]
        output: DiffOutputFormat,

        /// Exit with code 1 if any errors found
        #[arg(long)]
        strict: bool,
    },
}

#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    Yaml,
    Json,
    Html,
}

#[derive(Clone, ValueEnum)]
pub enum DiffOutputFormat {
    Terminal,
    Json,
    Yaml,
}
