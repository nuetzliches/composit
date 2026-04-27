//! Shared YAML → JSON conversion helpers used by multiple scanners.
//!
//! Lives in `core` rather than under `scanners/` because the same helper
//! shape would otherwise be duplicated per scanner module — and the next
//! scanner that needs to surface a `map[string]string` field (Helm
//! `values.yaml`, Kustomize `commonLabels`, …) should reach for the same
//! function instead of reinventing the coercion rules.

use serde_yaml::Value;

/// Convert a `serde_yaml::Mapping` whose keys are strings into a
/// `serde_json::Map<String, String-as-Value>`.
///
/// Designed for K8s-style label / annotation maps where the wire spec is
/// `map[string]string`. YAML can still serialize numeric or boolean
/// scalars in those positions (e.g. `replicas: 3` or `enabled: true`
/// without quotes); this helper coerces those to their string form so an
/// unquoted value still surfaces in the report instead of silently
/// disappearing. Nested maps, sequences, and null values are skipped —
/// they are not valid in label/annotation positions.
pub fn yaml_string_map_to_json(
    map: &serde_yaml::Mapping,
) -> serde_json::Map<String, serde_json::Value> {
    let mut out = serde_json::Map::new();
    for (k, v) in map {
        let Some(ks) = k.as_str() else { continue };
        let value_str = match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        };
        if let Some(vs) = value_str {
            out.insert(ks.to_string(), serde_json::Value::String(vs));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml_mapping(src: &str) -> serde_yaml::Mapping {
        let v: serde_yaml::Value = serde_yaml::from_str(src).unwrap();
        v.as_mapping().unwrap().clone()
    }

    #[test]
    fn coerces_numbers_and_bools_to_strings() {
        // YAML without quotes yields Number/Bool variants. The helper must
        // surface them as strings so an operator who wrote `tier: 1`
        // still sees the label, instead of having it silently dropped.
        let m = yaml_mapping(
            r#"
app: api
replicas: 3
ratio: 1.5
enabled: true
"#,
        );
        let out = yaml_string_map_to_json(&m);
        assert_eq!(out.get("app").and_then(|v| v.as_str()), Some("api"));
        assert_eq!(out.get("replicas").and_then(|v| v.as_str()), Some("3"));
        assert_eq!(out.get("ratio").and_then(|v| v.as_str()), Some("1.5"));
        assert_eq!(out.get("enabled").and_then(|v| v.as_str()), Some("true"));
    }

    #[test]
    fn skips_nested_maps_and_sequences() {
        // Nested structures aren't valid in label/annotation positions.
        // Drop them rather than emit a confusing JSON shape.
        let m = yaml_mapping(
            r#"
flat: yes
nested:
  inner: value
list:
  - a
  - b
"#,
        );
        let out = yaml_string_map_to_json(&m);
        assert_eq!(out.len(), 1);
        assert_eq!(out.get("flat").and_then(|v| v.as_str()), Some("yes"));
    }

    #[test]
    fn skips_non_string_keys() {
        // YAML allows numeric keys; K8s metadata does not. The helper
        // must drop non-string keys without panicking.
        let m = yaml_mapping(
            r#"
1: numeric-key
text: ok
"#,
        );
        let out = yaml_string_map_to_json(&m);
        assert_eq!(out.len(), 1);
        assert!(out.contains_key("text"));
    }
}
