use anyhow::Result;

use crate::core::types::Report;

pub fn to_yaml(report: &Report) -> Result<String> {
    let yaml = serde_yaml::to_string(report)?;
    Ok(yaml)
}
