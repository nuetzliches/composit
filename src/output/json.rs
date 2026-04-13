use anyhow::Result;

use crate::core::types::Report;

pub fn to_json(report: &Report) -> Result<String> {
    let json = serde_json::to_string_pretty(report)?;
    Ok(json)
}
