//! Integration tests that pin the composit-report v0.1 file format.
//! Guards against silent schema drift between the Rust types, the example,
//! and the published JSON Schema.

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn example_report_matches_required_top_level_shape() {
    // Integration tests can't access internal types (no lib target), so we
    // assert the top-level shape via raw YAML. Guards against an example
    // that silently drifts from the published schema.
    let path = repo_root().join("examples/composit-report.yaml");
    let content =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let doc: serde_yaml::Value = serde_yaml::from_str(&content)
        .unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e));

    let map = doc.as_mapping().expect("example report must be a mapping");
    for field in &[
        "workspace",
        "generated",
        "scanner_version",
        "providers",
        "resources",
        "summary",
    ] {
        assert!(
            map.contains_key(serde_yaml::Value::from(*field)),
            "example report missing required top-level field: {}",
            field
        );
    }

    // Summary must carry the computed attribution counters.
    let summary = map
        .get(serde_yaml::Value::from("summary"))
        .and_then(|v| v.as_mapping())
        .expect("summary is a mapping");
    for field in &[
        "total_resources",
        "providers",
        "agent_created",
        "agent_assisted",
        "human_created",
        "auto_detected",
        "estimated_monthly_cost",
    ] {
        assert!(
            summary.contains_key(serde_yaml::Value::from(*field)),
            "summary missing required field: {}",
            field
        );
    }
}

#[test]
fn schema_allows_x_prefix_extensions_on_root_and_summary() {
    // The schema declares additionalProperties: false at root and summary.
    // Without patternProperties "^x-", extensions would be rejected.
    // This test pins both locations so future edits don't silently close
    // the extension surface.
    let path = repo_root().join("schemas/composit-report-v0.1.json");
    let content = fs::read_to_string(&path).unwrap();
    let schema: serde_json::Value = serde_json::from_str(&content).unwrap();

    fn has_x_pattern(obj: &serde_json::Value) -> bool {
        obj.get("patternProperties")
            .and_then(|v| v.as_object())
            .map(|m| m.contains_key("^x-"))
            .unwrap_or(false)
    }

    assert!(
        has_x_pattern(&schema),
        "root object must accept ^x- extensions"
    );

    let summary = schema
        .pointer("/$defs/Summary")
        .expect("Summary definition present");
    assert!(
        has_x_pattern(summary),
        "Summary object must accept ^x- extensions"
    );
}

#[test]
fn json_schema_is_valid_json() {
    let path = repo_root().join("schemas/composit-report-v0.1.json");
    let content =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let schema: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e));

    assert_eq!(
        schema.get("$schema").and_then(|v| v.as_str()),
        Some("https://json-schema.org/draft/2020-12/schema"),
        "schema must declare Draft 2020-12"
    );
    assert_eq!(
        schema.get("title").and_then(|v| v.as_str()),
        Some("composit-report")
    );

    // Required top-level fields should match the Rust Report struct.
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .expect("required array present")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for field in &[
        "workspace",
        "generated",
        "scanner_version",
        "providers",
        "resources",
        "summary",
    ] {
        assert!(
            required.contains(field),
            "schema missing required field: {}",
            field
        );
    }
}

#[test]
fn public_provider_manifest_example_matches_schema_shape() {
    // RFC 002 v0.1: the example public manifest must stay minimal —
    // no tools / description / repo / license on capabilities, and
    // every required top-level field must be present.
    let manifest_path = repo_root().join("examples/composit-manifest.json");
    let content = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("read {}: {}", manifest_path.display(), e));
    let doc: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("parse {}: {}", manifest_path.display(), e));

    for field in &["composit", "provider", "capabilities"] {
        assert!(
            doc.get(field).is_some(),
            "example public manifest missing required field: {}",
            field
        );
    }

    let caps = doc
        .get("capabilities")
        .and_then(|v| v.as_array())
        .expect("capabilities is an array");
    for cap in caps {
        let obj = cap.as_object().expect("capability is an object");
        for forbidden in &["tools", "description", "repo", "license"] {
            assert!(
                !obj.contains_key(*forbidden),
                "capability should not leak {} at the public tier (RFC 002)",
                forbidden
            );
        }
    }

    // Contracts pointer shape: url + auth.type.
    let contracts = doc
        .get("contracts")
        .and_then(|v| v.as_array())
        .expect("contracts[] present");
    assert!(
        !contracts.is_empty(),
        "contracts[] must have at least one pointer"
    );
    for c in contracts {
        assert!(c.get("url").and_then(|v| v.as_str()).is_some());
        let auth_type = c
            .pointer("/auth/type")
            .and_then(|v| v.as_str())
            .expect("contracts[].auth.type");
        assert!(
            matches!(auth_type, "api-key" | "oauth2"),
            "unexpected auth.type in example: {}",
            auth_type
        );
    }
}

#[test]
fn public_provider_manifest_schema_is_valid_json() {
    let path = repo_root().join("schemas/composit-provider-manifest-v0.1.json");
    let content =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let schema: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e));

    assert_eq!(
        schema.get("$schema").and_then(|v| v.as_str()),
        Some("https://json-schema.org/draft/2020-12/schema"),
        "schema must declare Draft 2020-12"
    );
    assert_eq!(
        schema.get("title").and_then(|v| v.as_str()),
        Some("composit-provider-manifest")
    );

    // Top-level required fields: composit, provider, capabilities.
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .expect("required array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for field in &["composit", "provider", "capabilities"] {
        assert!(
            required.contains(field),
            "public-manifest schema missing required field: {}",
            field
        );
    }

    // PublicCapability must NOT declare tools/description/repo/license as
    // allowed properties — those belong in the product's own manifest or
    // in the contract tier (RFC 002).
    let cap_props = schema
        .pointer("/$defs/PublicCapability/properties")
        .and_then(|v| v.as_object())
        .expect("PublicCapability.properties");
    for forbidden in &["tools", "description", "repo", "license"] {
        assert!(
            !cap_props.contains_key(*forbidden),
            "PublicCapability must not allow `{}` — belongs in contract tier",
            forbidden
        );
    }
}

#[test]
fn contract_response_schema_is_valid_json() {
    // RFC 003: the contract-response schema must declare draft/2020-12 and
    // pin the required `contract.{id, provider, issued_at, expires_at}`
    // envelope so external provider implementers have a stable target.
    let path = repo_root().join("schemas/composit-contract-response-v0.1.json");
    let content =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let schema: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e));

    assert_eq!(
        schema.get("$schema").and_then(|v| v.as_str()),
        Some("https://json-schema.org/draft/2020-12/schema"),
        "schema must declare Draft 2020-12"
    );
    assert_eq!(
        schema.get("title").and_then(|v| v.as_str()),
        Some("composit-contract-response")
    );

    // Top-level required fields: composit, contract.
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .expect("required array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for field in &["composit", "contract"] {
        assert!(
            required.contains(field),
            "contract-response schema missing required field: {}",
            field
        );
    }

    // Contract object must require the v0.1 bookkeeping fields.
    let contract_required: Vec<&str> = schema
        .pointer("/$defs/Contract/required")
        .and_then(|v| v.as_array())
        .expect("Contract.required")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for field in &["id", "provider", "issued_at", "expires_at"] {
        assert!(
            contract_required.contains(field),
            "Contract.required missing: {}",
            field
        );
    }
}

#[test]
fn contract_response_example_matches_schema_shape() {
    // RFC 003: the example response must carry the v0.1 required fields
    // so it stays a usable target for provider implementers.
    let path = repo_root().join("examples/composit-contract.example.json");
    let content =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let doc: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e));

    for field in &["composit", "contract"] {
        assert!(
            doc.get(field).is_some(),
            "example contract response missing required top-level field: {}",
            field
        );
    }

    for field in &["id", "provider", "issued_at", "expires_at"] {
        assert!(
            doc.pointer(&format!("/contract/{}", field)).is_some(),
            "example contract missing required field: contract.{}",
            field
        );
    }

    // Capabilities SHOULD mirror the public manifest's types when present.
    if let Some(caps) = doc.get("capabilities").and_then(|v| v.as_array()) {
        assert!(
            !caps.is_empty(),
            "example capabilities[] should not be empty when the key is present"
        );
        for cap in caps {
            assert!(
                cap.pointer("/type").and_then(|v| v.as_str()).is_some(),
                "every contract capability must carry a `type`"
            );
        }
    }
}
