use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use super::types::{Provider, Resource};

pub struct ScanContext {
    pub dir: PathBuf,
    /// Provider targets to contact during the network phase. Each entry
    /// carries the public-manifest URL plus optional trust/auth metadata
    /// derived from a Compositfile. See RFC 002.
    pub providers: Vec<ProviderTarget>,
    pub skip_providers: bool,
}

/// A provider to scan, with optional RFC-002 contract hints.
#[derive(Debug, Clone)]
pub struct ProviderTarget {
    /// Public manifest URL — must resolve to `/.well-known/composit.json`.
    pub url: String,
    /// Trust level declared in the Compositfile: "public" or "contract".
    /// None when the target was discovered opportunistically (e.g. from
    /// an MCP config) and no governance is attached.
    pub trust: Option<String>,
    /// Auth method the Compositfile expects — currently only `"api-key"`.
    pub auth_type: Option<String>,
    /// Name of the environment variable that holds the credential value.
    /// The scanner reads this lazily at request time.
    pub auth_env: Option<String>,
}

impl ProviderTarget {
    /// Convenience constructor for URL-only targets (public-tier only,
    /// no governance attached).
    pub fn public_only(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            trust: None,
            auth_type: None,
            auth_env: None,
        }
    }
}

pub struct ScanResult {
    pub resources: Vec<Resource>,
    pub providers: Vec<Provider>,
}

#[async_trait]
#[allow(dead_code)]
pub trait Scanner: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn needs_network(&self) -> bool;

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult>;
}
