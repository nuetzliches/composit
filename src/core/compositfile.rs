use std::path::Path;

use anyhow::{anyhow, Context, Result};
use hcl::Body;

use crate::core::governance::{
    AllowRule, AuthRef, BudgetRule, Governance, PolicyRule, ProviderRule, RequireRule,
    ResourceConstraints,
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
                eprintln!("Warning: unknown block type '{}' in Compositfile", other);
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

    // Optional nested `auth { type = "...", env = "..." }` block.
    let auth = block
        .body
        .blocks()
        .find(|b| b.identifier.as_str() == "auth")
        .map(|b| parse_auth_block(b, &name))
        .transpose()?;

    // RFC 002 § "Compositfile extensions": trust="contract" requires auth.
    // trust="public" MUST NOT carry an auth block (signals intent confusion).
    match (trust.as_str(), auth.is_some()) {
        ("contract", false) => {
            return Err(anyhow!(
                "Provider '{}' declares trust = \"contract\" but has no auth block. \
                 Add: auth {{ type = \"api-key\", env = \"SOME_ENV_VAR\" }}",
                name
            ));
        }
        ("public", true) => {
            return Err(anyhow!(
                "Provider '{}' declares trust = \"public\" but also defines an auth block. \
                 Remove the auth block or switch trust to \"contract\".",
                name
            ));
        }
        _ => {}
    }

    Ok(ProviderRule {
        name,
        manifest,
        trust,
        compliance,
        auth,
    })
}

fn parse_auth_block(block: &hcl::Block, provider_name: &str) -> Result<AuthRef> {
    let auth_type = get_string_attr(&block.body, "type").ok_or_else(|| {
        anyhow!(
            "Provider '{}' auth block missing 'type' (e.g. \"api-key\")",
            provider_name
        )
    })?;

    // v0.1: only api-key is normative. oauth2 is reserved for the RFC 002
    // roadmap but rejected here until the CLI grows the fetch path.
    match auth_type.as_str() {
        "api-key" => {}
        "oauth2" => {
            return Err(anyhow!(
                "Provider '{}' declares auth.type = \"oauth2\"; that method is on the RFC 002 \
                 roadmap but not yet implemented. Use \"api-key\" for now.",
                provider_name
            ));
        }
        other => {
            return Err(anyhow!(
                "Provider '{}' auth.type = \"{}\" is not recognised. Valid: \"api-key\".",
                provider_name,
                other
            ));
        }
    }

    let env = get_string_attr(&block.body, "env");

    Ok(AuthRef { auth_type, env })
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
                eprintln!("Warning: unknown block '{}' in resources block", other);
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
                auth {
                  type = "api-key"
                  env  = "CRONIQ_COMPOSIT_KEY"
                }
              }

              provider "hookaido" {
                manifest = "https://hooks.example.com/.well-known/composit.json"
                trust    = "contract"
                auth {
                  type = "api-key"
                  env  = "HOOKAIDO_COMPOSIT_KEY"
                }
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
        let auth = gov.providers[0].auth.as_ref().expect("auth block present");
        assert_eq!(auth.auth_type, "api-key");
        assert_eq!(auth.env.as_deref(), Some("CRONIQ_COMPOSIT_KEY"));
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
    fn test_parse_public_trust_without_auth() {
        // trust="public" MUST NOT carry an auth block but MAY omit it.
        let gov = parse_hcl(
            r#"
            workspace "test" {
              provider "croniq" {
                manifest = "https://example.com/.well-known/composit.json"
                trust    = "public"
              }
            }
            "#,
        )
        .unwrap();

        assert_eq!(gov.providers.len(), 1);
        assert_eq!(gov.providers[0].trust, "public");
        assert!(gov.providers[0].auth.is_none());
    }

    #[test]
    fn test_contract_trust_without_auth_errors() {
        let err = parse_hcl(
            r#"
            workspace "test" {
              provider "croniq" {
                manifest = "https://example.com/.well-known/composit.json"
                trust    = "contract"
              }
            }
            "#,
        )
        .expect_err("contract trust without auth must fail");
        let msg = format!("{}", err);
        assert!(
            msg.contains("auth") && msg.contains("croniq"),
            "error should mention the provider name and the missing auth block: {}",
            msg
        );
    }

    #[test]
    fn test_public_trust_with_auth_errors() {
        let err = parse_hcl(
            r#"
            workspace "test" {
              provider "croniq" {
                manifest = "https://example.com/.well-known/composit.json"
                trust    = "public"
                auth {
                  type = "api-key"
                  env  = "X"
                }
              }
            }
            "#,
        )
        .expect_err("public trust with auth block must fail");
        let msg = format!("{}", err);
        assert!(
            msg.contains("public") && msg.contains("auth"),
            "error should flag the public+auth contradiction: {}",
            msg
        );
    }

    #[test]
    fn test_oauth2_rejected_in_v0_1() {
        let err = parse_hcl(
            r#"
            workspace "test" {
              provider "croniq" {
                manifest = "https://example.com/.well-known/composit.json"
                trust    = "contract"
                auth {
                  type = "oauth2"
                }
              }
            }
            "#,
        )
        .expect_err("oauth2 is on the roadmap, not implemented");
        let msg = format!("{}", err);
        assert!(msg.contains("oauth2") || msg.contains("OAuth"));
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
