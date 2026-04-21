use anyhow::Result;

use super::governance::ScanSettings;
use super::scanner::{ProviderTarget, ScanContext, ScanResult, Scanner};
use super::types::{Provider, Resource};

pub struct ScannerRegistry {
    scanners: Vec<Box<dyn Scanner>>,
}

impl ScannerRegistry {
    pub fn new() -> Self {
        ScannerRegistry {
            scanners: Vec::new(),
        }
    }

    pub fn register(&mut self, scanner: Box<dyn Scanner>) {
        self.scanners.push(scanner);
    }

    /// Run all applicable scanners. Filesystem scanners run first,
    /// then network scanners receive discovered provider URLs.
    pub async fn run_all(
        &self,
        context: &ScanContext,
        scan: Option<&ScanSettings>,
    ) -> Result<ScanResult> {
        let mut all_resources: Vec<Resource> = Vec::new();
        let mut all_providers: Vec<Provider> = Vec::new();

        // Phase 1: filesystem scanners (needs_network == false)
        let fs_scanners: Vec<&dyn Scanner> = self
            .scanners
            .iter()
            .filter(|s| !s.needs_network())
            .filter(|s| scan.map(|c| c.is_scanner_enabled(s.id())).unwrap_or(true))
            .map(|s| s.as_ref())
            .collect();

        for scanner in &fs_scanners {
            match scanner.scan(context).await {
                Ok(result) => {
                    all_resources.extend(result.resources);
                    all_providers.extend(result.providers);
                }
                Err(e) => {
                    eprintln!("Warning: scanner '{}' failed: {}", scanner.id(), e);
                }
            }
        }

        // Phase 2: network scanners (if not skipped)
        if !context.skip_providers {
            let net_scanners: Vec<&dyn Scanner> = self
                .scanners
                .iter()
                .filter(|s| s.needs_network())
                .filter(|s| scan.map(|c| c.is_scanner_enabled(s.id())).unwrap_or(true))
                .map(|s| s.as_ref())
                .collect();

            // Build extended target list with discovered provider URLs.
            // Entries carry optional trust/auth metadata from the initial
            // context (typically sourced from a Compositfile). Discovered
            // URLs are treated as public-only.
            let mut extended_providers: Vec<ProviderTarget> = context.providers.clone();
            let known_urls = |list: &[ProviderTarget], url: &str| list.iter().any(|t| t.url == url);
            for p in &all_providers {
                if !known_urls(&extended_providers, &p.endpoint) {
                    extended_providers.push(ProviderTarget::public_only(p.endpoint.clone()));
                }
            }

            let extended_context = ScanContext {
                dir: context.dir.clone(),
                providers: extended_providers,
                skip_providers: false,
                exclude_patterns: context.exclude_patterns.clone(),
            };

            for scanner in &net_scanners {
                match scanner.scan(&extended_context).await {
                    Ok(result) => {
                        all_resources.extend(result.resources);
                        all_providers.extend(result.providers);
                    }
                    Err(e) => {
                        eprintln!("Warning: scanner '{}' failed: {}", scanner.id(), e);
                    }
                }
            }
        }

        Ok(ScanResult {
            resources: all_resources,
            providers: all_providers,
        })
    }
}
