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
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
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
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
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
