use std::path::Path;

use anyhow::{anyhow, Context, Result};
use hcl::Body;

use crate::core::governance::{
    AllowRule, BudgetRule, Governance, PolicyRule, ProviderRule, RequireRule, ResourceConstraints,
};

/// Parse a Compositfile (HCL governance document) into a Governance struct.
pub fn parse_compositfile(path: &Path) -> Result<Governance> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read Compositfile: {}", path.display()))?;

    let body: Body = hcl::from_str(&content)
        .with_context(|| format!("Failed to parse Compositfile HCL: {}", path.display()))?;

    let workspace_block = body
        .blocks()
        .find(|b| b.identifier.as_str() == "workspace")
        .ok_or_else(|| anyhow!("No 'workspace' block found in Compositfile"))?;

    let workspace_name = workspace_block
        .labels
        .first()
        .map(|l| l.as_str().to_string())
        .unwrap_or_else(|| "unnamed".to_string());

    let mut providers = Vec::new();
    let mut budgets = Vec::new();
    let mut policies = Vec::new();
    let mut resources = None;

    for block in workspace_block.body.blocks() {
        match block.identifier.as_str() {
            "provider" => providers.push(parse_provider_block(block)?),
            "budget" => budgets.push(parse_budget_block(block)?),
            "policy" => policies.push(parse_policy_block(block)?),
            "resources" => resources = Some(parse_resources_block(block)?),
            other => {
                eprintln!(
                    "Warning: unknown block type '{}' in Compositfile",
                    other
                );
            }
        }
    }

    Ok(Governance {
        workspace: workspace_name,
        providers,
        budgets,
        policies,
        resources,
    })
}

fn parse_provider_block(block: &hcl::Block) -> Result<ProviderRule> {
    let name = block
        .labels
        .first()
        .map(|l| l.as_str().to_string())
        .unwrap_or_else(|| "unnamed".to_string());

    let manifest = get_string_attr(&block.body, "manifest")
        .ok_or_else(|| anyhow!("Provider '{}' missing 'manifest' attribute", name))?;
    let trust = get_string_attr(&block.body, "trust")
        .ok_or_else(|| anyhow!("Provider '{}' missing 'trust' attribute", name))?;
    let compliance = get_string_array_attr(&block.body, "compliance");

    Ok(ProviderRule {
        name,
        manifest,
        trust,
        compliance,
    })
}

fn parse_budget_block(block: &hcl::Block) -> Result<BudgetRule> {
    let scope = block
        .labels
        .first()
        .map(|l| l.as_str().to_string())
        .unwrap_or_else(|| "unnamed".to_string());

    let max_monthly = get_string_attr(&block.body, "max_monthly")
        .ok_or_else(|| anyhow!("Budget '{}' missing 'max_monthly' attribute", scope))?;
    let alert_at = get_string_attr(&block.body, "alert_at");

    Ok(BudgetRule {
        scope,
        max_monthly,
        alert_at,
    })
}

fn parse_policy_block(block: &hcl::Block) -> Result<PolicyRule> {
    let name = block
        .labels
        .first()
        .map(|l| l.as_str().to_string())
        .unwrap_or_else(|| "unnamed".to_string());

    let source = get_string_attr(&block.body, "source")
        .ok_or_else(|| anyhow!("Policy '{}' missing 'source' attribute", name))?;
    let description = get_string_attr(&block.body, "description");

    Ok(PolicyRule {
        name,
        source,
        description,
    })
}

fn parse_resources_block(block: &hcl::Block) -> Result<ResourceConstraints> {
    let max_total = get_usize_attr(&block.body, "max_total");
    let mut allow = Vec::new();
    let mut require = Vec::new();

    for inner in block.body.blocks() {
        match inner.identifier.as_str() {
            "allow" => {
                let resource_type = inner
                    .labels
                    .first()
                    .map(|l| l.as_str().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let max = get_usize_attr(&inner.body, "max");
                let allowed_images = get_string_array_attr(&inner.body, "allowed_images");
                let allowed_types = get_string_array_attr(&inner.body, "allowed_types");

                allow.push(AllowRule {
                    resource_type,
                    max,
                    allowed_images,
                    allowed_types,
                });
            }
            "require" => {
                let resource_type = inner
                    .labels
                    .first()
                    .map(|l| l.as_str().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let min = get_usize_attr(&inner.body, "min").unwrap_or(1);

                require.push(RequireRule { resource_type, min });
            }
            other => {
                eprintln!(
                    "Warning: unknown block '{}' in resources block",
                    other
                );
            }
        }
    }

    Ok(ResourceConstraints {
        max_total,
        allow,
        require,
    })
}

/// Extract a string attribute from an HCL body.
fn get_string_attr(body: &Body, key: &str) -> Option<String> {
    body.attributes()
        .find(|a| a.key.as_str() == key)
        .and_then(|a| match &a.expr {
            hcl::Expression::String(s) => Some(s.clone()),
            _ => None,
        })
}

/// Extract a string array attribute from an HCL body.
fn get_string_array_attr(body: &Body, key: &str) -> Vec<String> {
    body.attributes()
        .find(|a| a.key.as_str() == key)
        .map(|a| match &a.expr {
            hcl::Expression::Array(items) => items
                .iter()
                .filter_map(|e| match e {
                    hcl::Expression::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        })
        .unwrap_or_default()
}

/// Extract an unsigned integer attribute from an HCL body.
fn get_usize_attr(body: &Body, key: &str) -> Option<usize> {
    body.attributes()
        .find(|a| a.key.as_str() == key)
        .and_then(|a| match &a.expr {
            hcl::Expression::Number(n) => n.as_u64().map(|v| v as usize),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_hcl(content: &str) -> Result<Governance> {
        let body: Body = hcl::from_str(content)?;
        let workspace_block = body
            .blocks()
            .find(|b| b.identifier.as_str() == "workspace")
            .ok_or_else(|| anyhow!("No workspace block"))?;

        let workspace_name = workspace_block
            .labels
            .first()
            .map(|l| l.as_str().to_string())
            .unwrap_or_else(|| "unnamed".to_string());

        let mut providers = Vec::new();
        let mut budgets = Vec::new();
        let mut policies = Vec::new();
        let mut resources = None;

        for block in workspace_block.body.blocks() {
            match block.identifier.as_str() {
                "provider" => providers.push(parse_provider_block(block)?),
                "budget" => budgets.push(parse_budget_block(block)?),
                "policy" => policies.push(parse_policy_block(block)?),
                "resources" => resources = Some(parse_resources_block(block)?),
                _ => {}
            }
        }

        Ok(Governance {
            workspace: workspace_name,
            providers,
            budgets,
            policies,
            resources,
        })
    }

    #[test]
    fn test_parse_full_compositfile() {
        let gov = parse_hcl(
            r#"
            workspace "test" {
              provider "croniq" {
                manifest = "https://example.com/.well-known/composit.json"
                trust    = "contract"
                compliance = ["gdpr", "eu-ai-act"]
              }

              provider "hookaido" {
                manifest = "https://hooks.example.com/.well-known/composit.json"
                trust    = "contract"
              }

              budget "workspace" {
                max_monthly = "500 EUR"
                alert_at    = "80%"
              }

              policy "limits" {
                source      = "policies/limits.rego"
                description = "Resource limits"
              }

              resources {
                max_total = 100

                allow "docker_service" {
                  max = 20
                }

                allow "workflow" {
                  max = 10
                }

                require "docker_compose" {
                  min = 1
                }
              }
            }
            "#,
        )
        .unwrap();

        assert_eq!(gov.workspace, "test");
        assert_eq!(gov.providers.len(), 2);
        assert_eq!(gov.providers[0].name, "croniq");
        assert_eq!(gov.providers[0].compliance, vec!["gdpr", "eu-ai-act"]);
        assert_eq!(gov.providers[1].compliance.len(), 0);
        assert_eq!(gov.budgets.len(), 1);
        assert_eq!(gov.budgets[0].max_monthly, "500 EUR");
        assert_eq!(gov.budgets[0].alert_at.as_deref(), Some("80%"));
        assert_eq!(gov.policies.len(), 1);

        let res = gov.resources.unwrap();
        assert_eq!(res.max_total, Some(100));
        assert_eq!(res.allow.len(), 2);
        assert_eq!(res.allow[0].resource_type, "docker_service");
        assert_eq!(res.allow[0].max, Some(20));
        assert_eq!(res.require.len(), 1);
        assert_eq!(res.require[0].resource_type, "docker_compose");
        assert_eq!(res.require[0].min, 1);
    }

    #[test]
    fn test_parse_minimal_compositfile() {
        let gov = parse_hcl(
            r#"
            workspace "minimal" {
            }
            "#,
        )
        .unwrap();

        assert_eq!(gov.workspace, "minimal");
        assert!(gov.providers.is_empty());
        assert!(gov.budgets.is_empty());
        assert!(gov.policies.is_empty());
        assert!(gov.resources.is_none());
    }

    #[test]
    fn test_missing_workspace_block() {
        let result = parse_hcl(
            r#"
            provider "test" {
              manifest = "https://example.com"
              trust    = "open"
            }
            "#,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_require_default_min() {
        let gov = parse_hcl(
            r#"
            workspace "test" {
              resources {
                require "workflow" {
                }
              }
            }
            "#,
        )
        .unwrap();

        let res = gov.resources.unwrap();
        assert_eq!(res.require[0].min, 1);
    }

    #[test]
    fn test_budget_without_alert() {
        let gov = parse_hcl(
            r#"
            workspace "test" {
              budget "session" {
                max_monthly = "50 EUR"
              }
            }
            "#,
        )
        .unwrap();

        assert_eq!(gov.budgets[0].scope, "session");
        assert!(gov.budgets[0].alert_at.is_none());
    }
}
