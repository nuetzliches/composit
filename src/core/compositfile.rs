use std::path::Path;

use anyhow::{anyhow, Context, Result};
use hcl::Body;

use crate::core::governance::{
    AllowRule, AnsibleSettings, AuthRef, BudgetRule, ExtraPattern, Governance, Matcher, PolicyRule,
    Predicate, ProviderRule, RequireRule, ResourceConstraints, Role, ScanSettings,
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
    let mut scan = ScanSettings::default();

    for block in workspace_block.body.blocks() {
        match block.identifier.as_str() {
            "provider" => providers.push(parse_provider_block(block)?),
            "budget" => budgets.push(parse_budget_block(block)?),
            "policy" => policies.push(parse_policy_block(block)?),
            "resources" => resources = Some(parse_resources_block(block)?),
            "scan" => scan = parse_scan_block(block)?,
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
        scan,
    })
}

/// Parse a `scan { exclude = [...]; extra_patterns { … }; scanners { prometheus = false } }`
/// block. All sub-fields are optional — an empty `scan { }` block is a
/// valid no-op.
fn parse_scan_block(block: &hcl::Block) -> Result<ScanSettings> {
    let exclude_paths = get_string_array_attr(&block.body, "exclude");
    let resolvable = get_string_array_attr(&block.body, "resolvable");
    let redact = get_string_array_attr(&block.body, "redact");

    let mut extra_patterns = Vec::new();
    for inner in block.body.blocks() {
        match inner.identifier.as_str() {
            "extra_patterns" => {
                let resource_type = get_string_attr(&inner.body, "type")
                    .ok_or_else(|| anyhow!("scan.extra_patterns block missing 'type' attribute"))?;
                let glob = get_string_attr(&inner.body, "glob")
                    .ok_or_else(|| anyhow!("scan.extra_patterns block missing 'glob' attribute"))?;
                let description = get_string_attr(&inner.body, "description");
                extra_patterns.push(ExtraPattern {
                    resource_type,
                    glob,
                    description,
                });
            }
            "scanners" => {
                // handled below
            }
            "ansible" => {
                // handled in its own pass below
            }
            other => {
                eprintln!("Warning: unknown block type '{}' inside scan {{ }}", other);
            }
        }
    }

    // RFC 007: optional `ansible { extra_vars { … }; inventories = [...] }` sub-block.
    let mut ansible = AnsibleSettings::default();
    if let Some(ansible_block) = block
        .body
        .blocks()
        .find(|b| b.identifier.as_str() == "ansible")
    {
        if let Some(ev_block) = ansible_block
            .body
            .blocks()
            .find(|b| b.identifier.as_str() == "extra_vars")
        {
            for attr in ev_block.body.attributes() {
                let key = attr.key.as_str().to_string();
                let value = match &attr.expr {
                    hcl::Expression::String(s) => s.clone(),
                    hcl::Expression::Number(n) => n.to_string(),
                    hcl::Expression::Bool(b) => b.to_string(),
                    _ => continue,
                };
                ansible.extra_vars.insert(key, value);
            }
        }
        ansible.inventories = get_string_array_attr(&ansible_block.body, "inventories");
    }

    // `scanners { prometheus = false }` maps each attribute to an on/off
    // toggle. Missing keys default to "enabled" elsewhere.
    let mut scanners = std::collections::HashMap::new();
    if let Some(scanners_block) = block
        .body
        .blocks()
        .find(|b| b.identifier.as_str() == "scanners")
    {
        for attr in scanners_block.body.attributes() {
            if let Some(b) = attr_as_bool(&attr.expr) {
                scanners.insert(attr.key.as_str().to_string(), b);
            }
        }
    }

    Ok(ScanSettings {
        exclude_paths,
        extra_patterns,
        scanners,
        resolvable,
        redact,
        ansible,
    })
}

fn attr_as_bool(expr: &hcl::Expression) -> Option<bool> {
    match expr {
        hcl::Expression::Bool(b) => Some(*b),
        _ => None,
    }
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

    if let Some(raw) = &alert_at {
        validate_percentage(raw)
            .map_err(|e| anyhow!("Budget '{}' has invalid alert_at \"{}\": {}", scope, raw, e))?;
    }

    Ok(BudgetRule {
        scope,
        max_monthly,
        alert_at,
    })
}

/// Validate that a percentage literal is well-formed and in the `0-100%` range.
/// Matches the runtime parser in `commands::diff::parse_percentage` so a value
/// that survives `parse_compositfile` will also be honoured at diff time
/// instead of silently collapsing to `None`.
fn validate_percentage(s: &str) -> Result<(), String> {
    let trimmed = s.trim();
    let digits = trimmed
        .strip_suffix('%')
        .ok_or_else(|| "must end with '%' (e.g. \"80%\")".to_string())?;
    let value: f64 = digits
        .parse()
        .map_err(|_| format!("\"{}\" is not a number", digits))?;
    if !(0.0..=100.0).contains(&value) {
        return Err(format!("{}% is outside the 0-100% range", value));
    }
    Ok(())
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

                // RFC 005: `role "<name>" { … }` sub-blocks.
                let mut roles: Vec<Role> = Vec::new();
                for role_block in inner.body.blocks() {
                    if role_block.identifier.as_str() == "role" {
                        let role = parse_role_block(role_block, &resource_type)?;
                        if roles.iter().any(|r| r.name == role.name) {
                            return Err(anyhow!(
                                "Duplicate role \"{}\" in allow \"{}\" block",
                                role.name,
                                resource_type
                            ));
                        }
                        roles.push(role);
                    }
                }

                allow.push(AllowRule {
                    resource_type,
                    max,
                    allowed_images,
                    allowed_types,
                    roles,
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

/// Parse a `role "<name>" { match { … } image_pin = […] … }` block.
/// See RFC 005 for the attribute catalog.
fn parse_role_block(block: &hcl::Block, parent_type: &str) -> Result<Role> {
    let name = block
        .labels
        .first()
        .map(|l| l.as_str().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "role block inside allow \"{}\" has an empty or missing label",
                parent_type
            )
        })?;

    // Optional `match { name = [...], image = [...], path = [...], predicate = "all"|"any" }`.
    let matcher = match block
        .body
        .blocks()
        .find(|b| b.identifier.as_str() == "match")
    {
        Some(mb) => parse_match_block(mb, &name)?,
        None => Matcher::default(),
    };

    let image_pin = get_string_array_attr(&block.body, "image_pin");
    let image_prefix = get_string_array_attr(&block.body, "image_prefix");
    let must_expose = get_u16_array_attr(&block.body, "must_expose").map_err(|e| {
        anyhow!(
            "role \"{}\" must_expose: {} (ports must be positive integers ≤ 65535)",
            name,
            e
        )
    })?;
    let must_attach_to = get_string_array_attr(&block.body, "must_attach_to");
    let must_set_env = get_string_array_attr(&block.body, "must_set_env");
    let forbidden_env = get_string_array_attr(&block.body, "forbidden_env");
    let must_have_file = get_string_array_attr(&block.body, "must_have_file");
    let min_count = get_usize_attr(&block.body, "min_count");
    let max_count = get_usize_attr(&block.body, "max_count");

    // RFC 007: `rendered_must_contain { key = "glob" }` sub-block.
    let mut rendered_must_contain: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    if let Some(rmc_block) = block
        .body
        .blocks()
        .find(|b| b.identifier.as_str() == "rendered_must_contain")
    {
        for attr in rmc_block.body.attributes() {
            let key = attr.key.as_str().to_string();
            if let hcl::Expression::String(s) = &attr.expr {
                rendered_must_contain.insert(key, s.clone());
            }
        }
    }

    // Forward-compat: warn on unknown attrs/blocks (but don't fail).
    const KNOWN_ROLE_ATTRS: &[&str] = &[
        "image_pin",
        "image_prefix",
        "must_expose",
        "must_attach_to",
        "must_set_env",
        "forbidden_env",
        "must_have_file",
        "min_count",
        "max_count",
        "rendered_must_contain",
    ];
    for attr in block.body.attributes() {
        if !KNOWN_ROLE_ATTRS.contains(&attr.key.as_str()) {
            eprintln!(
                "Warning: unknown attribute '{}' in role \"{}\"",
                attr.key.as_str(),
                name
            );
        }
    }
    const KNOWN_ROLE_SUB_BLOCKS: &[&str] = &["match", "rendered_must_contain"];
    for sub in block.body.blocks() {
        if !KNOWN_ROLE_SUB_BLOCKS.contains(&sub.identifier.as_str()) {
            eprintln!(
                "Warning: unknown block '{}' in role \"{}\"",
                sub.identifier.as_str(),
                name
            );
        }
    }

    Ok(Role {
        name,
        matcher,
        image_pin,
        image_prefix,
        must_expose,
        must_attach_to,
        must_set_env,
        forbidden_env,
        must_have_file,
        min_count,
        max_count,
        rendered_must_contain,
    })
}

fn parse_match_block(block: &hcl::Block, role_name: &str) -> Result<Matcher> {
    let name = get_string_array_attr(&block.body, "name");
    let image = get_string_array_attr(&block.body, "image");
    let path = get_string_array_attr(&block.body, "path");

    let predicate = match get_string_attr(&block.body, "predicate").as_deref() {
        None | Some("all") => Predicate::All,
        Some("any") => Predicate::Any,
        Some(other) => {
            return Err(anyhow!(
                "role \"{}\" match.predicate = \"{}\" is not recognised. Valid: \"all\" (default), \"any\".",
                role_name,
                other
            ));
        }
    };

    Ok(Matcher {
        name,
        image,
        path,
        predicate,
    })
}

/// Extract a `[1, 2, 3]` HCL integer array as `Vec<u16>`.
/// Returns Err if any element is not a positive integer in the `u16` range —
/// role `must_expose` rejects negative/overflowing port numbers at parse time.
fn get_u16_array_attr(body: &Body, key: &str) -> Result<Vec<u16>, String> {
    let Some(attr) = body.attributes().find(|a| a.key.as_str() == key) else {
        return Ok(Vec::new());
    };
    let items = match &attr.expr {
        hcl::Expression::Array(items) => items,
        _ => return Err(format!("{} is not an array", key)),
    };

    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match item {
            hcl::Expression::Number(n) => match n.as_u64() {
                Some(v) if v <= u16::MAX as u64 => out.push(v as u16),
                Some(v) => return Err(format!("{} out of u16 range", v)),
                None => return Err(format!("{} is not a non-negative integer", n)),
            },
            _ => return Err("non-integer value in port list".to_string()),
        }
    }
    Ok(out)
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
        let mut scan = ScanSettings::default();

        for block in workspace_block.body.blocks() {
            match block.identifier.as_str() {
                "provider" => providers.push(parse_provider_block(block)?),
                "budget" => budgets.push(parse_budget_block(block)?),
                "policy" => policies.push(parse_policy_block(block)?),
                "resources" => resources = Some(parse_resources_block(block)?),
                "scan" => scan = parse_scan_block(block)?,
                _ => {}
            }
        }

        Ok(Governance {
            workspace: workspace_name,
            providers,
            budgets,
            policies,
            resources,
            scan,
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

    #[test]
    fn test_budget_rejects_out_of_range_alert_at() {
        // "150%" used to be accepted silently — parse succeeded, diff quietly
        // dropped it. Parser now rejects at load time so authors see the bug
        // when they write the Compositfile, not weeks later.
        let err = parse_hcl(
            r#"
            workspace "test" {
              budget "workspace" {
                max_monthly = "500 EUR"
                alert_at    = "150%"
              }
            }
            "#,
        )
        .unwrap_err()
        .to_string();

        assert!(
            err.contains("alert_at") && err.contains("150"),
            "expected error to name alert_at and the bad value, got: {err}"
        );
    }

    #[test]
    fn test_budget_rejects_alert_at_without_percent_suffix() {
        let err = parse_hcl(
            r#"
            workspace "test" {
              budget "workspace" {
                max_monthly = "500 EUR"
                alert_at    = "80"
              }
            }
            "#,
        )
        .unwrap_err()
        .to_string();

        assert!(
            err.contains("%"),
            "error must mention percent suffix: {err}"
        );
    }

    #[test]
    fn test_budget_accepts_edge_percentages() {
        for value in &["0%", "100%", "42.5%"] {
            let src = format!(
                r#"
                workspace "test" {{
                  budget "workspace" {{
                    max_monthly = "500 EUR"
                    alert_at    = "{}"
                  }}
                }}
                "#,
                value
            );
            let gov =
                parse_hcl(&src).unwrap_or_else(|e| panic!("'{value}' should parse but got: {e}"));
            assert_eq!(gov.budgets[0].alert_at.as_deref(), Some(*value));
        }
    }

    #[test]
    fn test_scan_block_populates_exclude_patterns_and_extras() {
        let gov = parse_hcl(
            r#"
            workspace "test" {
              scan {
                exclude = ["tests/fixtures", "examples", "**/*.generated.yaml"]

                extra_patterns {
                  type = "terraform_module"
                  glob = "modules/**/*.tf"
                }

                scanners {
                  prometheus = false
                }
              }
            }
            "#,
        )
        .unwrap();

        assert_eq!(
            gov.scan.exclude_paths,
            vec![
                "tests/fixtures".to_string(),
                "examples".to_string(),
                "**/*.generated.yaml".to_string(),
            ]
        );
        assert_eq!(gov.scan.extra_patterns.len(), 1);
        assert_eq!(gov.scan.extra_patterns[0].resource_type, "terraform_module");
        assert_eq!(gov.scan.extra_patterns[0].glob, "modules/**/*.tf");
        assert_eq!(gov.scan.scanners.get("prometheus"), Some(&false));
        assert!(gov.scan.is_scanner_enabled("docker"));
    }

    #[test]
    fn test_parse_role_block_with_matcher_and_constraints() {
        let gov = parse_hcl(
            r#"
            workspace "test" {
              resources {
                allow "docker_service" {
                  max = 20

                  role "database" {
                    match {
                      name      = ["*postgres*", "*mssql*"]
                      image     = ["postgres:*"]
                      predicate = "any"
                    }
                    image_pin      = ["postgres:16", "postgres:17"]
                    must_expose    = [5432]
                    must_attach_to = ["backend"]
                    max_count      = 3
                  }

                  role "api" {
                    match { name = ["*-api"] }
                    image_prefix = ["git.example/acme/"]
                    must_set_env = ["DATABASE_URL"]
                  }
                }
              }
            }
            "#,
        )
        .unwrap();
        let res = gov.resources.unwrap();
        assert_eq!(res.allow.len(), 1);
        let allow = &res.allow[0];
        assert_eq!(allow.roles.len(), 2);

        let db = &allow.roles[0];
        assert_eq!(db.name, "database");
        assert_eq!(db.matcher.predicate, Predicate::Any);
        assert_eq!(db.matcher.name, vec!["*postgres*", "*mssql*"]);
        assert_eq!(db.image_pin, vec!["postgres:16", "postgres:17"]);
        assert_eq!(db.must_expose, vec![5432]);
        assert_eq!(db.must_attach_to, vec!["backend"]);
        assert_eq!(db.max_count, Some(3));

        let api = &allow.roles[1];
        assert_eq!(api.name, "api");
        assert_eq!(api.matcher.predicate, Predicate::All); // default
        assert_eq!(api.image_prefix, vec!["git.example/acme/"]);
        assert_eq!(api.must_set_env, vec!["DATABASE_URL"]);
    }

    #[test]
    fn test_duplicate_role_label_rejected() {
        let err = parse_hcl(
            r#"
            workspace "test" {
              resources {
                allow "docker_service" {
                  role "db" {
                    match {
                      name = ["*"]
                    }
                  }
                  role "db" {
                    match {
                      name = ["*"]
                    }
                  }
                }
              }
            }
            "#,
        )
        .expect_err("duplicate role labels must fail");
        assert!(
            format!("{}", err).contains("Duplicate role \"db\""),
            "wrong error: {}",
            err
        );
    }

    #[test]
    fn test_role_empty_label_rejected() {
        let err = parse_hcl(
            r#"
            workspace "test" {
              resources {
                allow "docker_service" {
                  role "" {
                    match {
                      name = ["*"]
                    }
                  }
                }
              }
            }
            "#,
        )
        .expect_err("empty role label must fail");
        assert!(
            format!("{}", err).contains("empty or missing label"),
            "wrong error: {}",
            err
        );
    }

    #[test]
    fn test_role_unknown_predicate_rejected() {
        let err = parse_hcl(
            r#"
            workspace "test" {
              resources {
                allow "docker_service" {
                  role "db" {
                    match {
                      name      = ["*"]
                      predicate = "maybe"
                    }
                  }
                }
              }
            }
            "#,
        )
        .expect_err("unknown predicate must fail");
        assert!(format!("{}", err).contains("maybe"), "wrong error: {}", err);
    }

    #[test]
    fn test_role_port_out_of_range_rejected() {
        let err = parse_hcl(
            r#"
            workspace "test" {
              resources {
                allow "docker_service" {
                  role "api" {
                    match {
                      name = ["*"]
                    }
                    must_expose = [70000]
                  }
                }
              }
            }
            "#,
        )
        .expect_err("port > 65535 must fail");
        assert!(
            format!("{}", err).contains("must_expose"),
            "wrong error: {}",
            err
        );
    }

    #[test]
    fn test_missing_scan_block_yields_default_empty_settings() {
        // A Compositfile without a scan block must still produce a valid
        // Governance — governance and scan tuning are independently optional.
        let gov = parse_hcl(
            r#"
            workspace "test" {
              resources { max_total = 10 }
            }
            "#,
        )
        .unwrap();
        assert!(gov.scan.exclude_paths.is_empty());
        assert!(gov.scan.extra_patterns.is_empty());
        assert!(gov.scan.scanners.is_empty());
    }
}
