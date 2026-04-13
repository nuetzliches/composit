use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use super::types::{Provider, Resource};

pub struct ScanContext {
    pub dir: PathBuf,
    pub providers: Vec<String>,
    pub skip_providers: bool,
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
