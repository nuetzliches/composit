//! Issue #20: opt-in provenance promotion.
//!
//! Layer 1 of the issue surfaces every label/annotation a scanner sees in
//! `Resource.extra` verbatim. Layer 2 — implemented here — promotes a
//! configured subset of those keys to a structured `provenance` block on
//! the resource so reporters can attribute drift back to the upstream spec
//! that emitted the artefact.
//!
//! The configured key lists live in `ScanSettings.provenance_labels` and
//! `ScanSettings.provenance_annotations`. Resolution runs as a post-scan
//! pass in `registry::run_all` (see `apply_provenance`) so individual
//! scanners stay oblivious to the feature.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::core::types::Resource;

/// Structured provenance block written to `Resource.extra["provenance"]`.
/// `BTreeMap` for `raw` keeps the JSON key order deterministic across
/// platforms — without it tests that compare reports byte-for-byte fail
/// intermittently.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Provenance {
    /// Value of the first matched `provenance_labels` entry. The list
    /// order in the Compositfile is the priority order, so authors can
    /// put the most-specific key first (e.g. `"vendor/source"` before
    /// `"app.kubernetes.io/managed-by"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,

    /// Value of the first matched `provenance_annotations` entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,

    /// Every matched key→value pair, regardless of which list (labels or
    /// annotations) it came from. Lets consumers see the full attribution
    /// surface without having to re-walk `extra.labels` / `extra.annotations`.
    pub raw: BTreeMap<String, String>,
}

impl Provenance {
    fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
}

/// Resolve provenance from a resource's already-extracted labels and
/// annotations against the configured key lists. Matching is exact-key
/// only in v1 — the issue lists concrete keys like
/// `"app.kubernetes.io/managed-by"` and glob support is a deliberate
/// follow-up. Returns `None` when nothing matches; callers should leave
/// `Resource.extra["provenance"]` unset in that case.
pub fn resolve(
    labels: Option<&serde_json::Map<String, serde_json::Value>>,
    annotations: Option<&serde_json::Map<String, serde_json::Value>>,
    label_keys: &[String],
    annotation_keys: &[String],
) -> Option<Provenance> {
    if label_keys.is_empty() && annotation_keys.is_empty() {
        return None;
    }
    let mut prov = Provenance {
        source_kind: None,
        source_ref: None,
        raw: BTreeMap::new(),
    };
    if let Some(labels) = labels {
        for key in label_keys {
            if let Some(value) = labels.get(key).and_then(|v| v.as_str()) {
                prov.raw.insert(key.clone(), value.to_string());
                if prov.source_kind.is_none() {
                    prov.source_kind = Some(value.to_string());
                }
            }
        }
    }
    if let Some(annotations) = annotations {
        for key in annotation_keys {
            if let Some(value) = annotations.get(key).and_then(|v| v.as_str()) {
                prov.raw.insert(key.clone(), value.to_string());
                if prov.source_ref.is_none() {
                    prov.source_ref = Some(value.to_string());
                }
            }
        }
    }
    if prov.is_empty() {
        None
    } else {
        Some(prov)
    }
}

/// Apply provenance resolution to every resource that carries `extra.labels`
/// or `extra.annotations`. Resources without either field are skipped — the
/// pass is a no-op when the Compositfile didn't opt into the feature.
pub fn apply_provenance(
    resources: &mut [Resource],
    label_keys: &[String],
    annotation_keys: &[String],
) {
    if label_keys.is_empty() && annotation_keys.is_empty() {
        return;
    }
    for r in resources.iter_mut() {
        let labels = r.extra.get("labels").and_then(|v| v.as_object()).cloned();
        let annotations = r
            .extra
            .get("annotations")
            .and_then(|v| v.as_object())
            .cloned();
        if labels.is_none() && annotations.is_none() {
            continue;
        }
        if let Some(prov) = resolve(
            labels.as_ref(),
            annotations.as_ref(),
            label_keys,
            annotation_keys,
        ) {
            if let Ok(value) = serde_json::to_value(&prov) {
                r.extra.insert("provenance".to_string(), value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_object<'a>(pairs: &[(&'a str, &'a str)]) -> serde_json::Map<String, serde_json::Value> {
        let mut m = serde_json::Map::new();
        for (k, v) in pairs {
            m.insert(
                (*k).to_string(),
                serde_json::Value::String((*v).to_string()),
            );
        }
        m
    }

    #[test]
    fn resolve_returns_none_when_no_patterns_configured() {
        // No opt-in → no provenance, even if labels exist. Keeps the
        // feature strictly off-by-default.
        let labels = json_object(&[("app.kubernetes.io/managed-by", "argocd")]);
        assert!(resolve(Some(&labels), None, &[], &[]).is_none());
    }

    #[test]
    fn resolve_returns_none_when_patterns_dont_match() {
        // Patterns configured but no actual label matches → None.
        let labels = json_object(&[("tier", "backend")]);
        let prov = resolve(
            Some(&labels),
            None,
            &["app.kubernetes.io/managed-by".to_string()],
            &[],
        );
        assert!(prov.is_none());
    }

    #[test]
    fn resolve_picks_first_matching_label_for_source_kind() {
        // Two label patterns both match. The one that comes first in the
        // configured list wins source_kind — operators can rank keys by
        // priority. raw has both entries.
        let labels = json_object(&[
            ("app.kubernetes.io/managed-by", "argocd"),
            ("vendor/source", "specs/api.yaml"),
        ]);
        let prov = resolve(
            Some(&labels),
            None,
            &[
                "vendor/source".to_string(),
                "app.kubernetes.io/managed-by".to_string(),
            ],
            &[],
        )
        .expect("provenance present");
        assert_eq!(prov.source_kind.as_deref(), Some("specs/api.yaml"));
        assert_eq!(prov.raw.len(), 2);
        assert_eq!(
            prov.raw.get("vendor/source").map(|s| s.as_str()),
            Some("specs/api.yaml")
        );
    }

    #[test]
    fn resolve_separates_kind_and_ref_across_label_and_annotation() {
        // source_kind comes from labels, source_ref from annotations —
        // the issue example splits them this way and reporters rely on
        // the distinction.
        let labels = json_object(&[("app.kubernetes.io/managed-by", "flux")]);
        let annotations = json_object(&[("vendor/workload-name", "api")]);
        let prov = resolve(
            Some(&labels),
            Some(&annotations),
            &["app.kubernetes.io/managed-by".to_string()],
            &["vendor/workload-name".to_string()],
        )
        .unwrap();
        assert_eq!(prov.source_kind.as_deref(), Some("flux"));
        assert_eq!(prov.source_ref.as_deref(), Some("api"));
    }

    #[test]
    fn apply_provenance_skips_resources_without_metadata() {
        // No labels, no annotations → resource untouched. Verifies the
        // post-scan pass is safe to run unconditionally.
        use std::collections::HashMap;
        let mut r = Resource {
            resource_type: "docker_service".to_string(),
            name: Some("api".to_string()),
            path: Some("./compose.yml".to_string()),
            provider: None,
            created: None,
            created_by: None,
            detected_by: "docker".to_string(),
            estimated_cost: None,
            extra: HashMap::new(),
        };
        let mut resources = vec![r.clone()];
        apply_provenance(
            &mut resources,
            &["app.kubernetes.io/managed-by".to_string()],
            &[],
        );
        assert!(!resources[0].extra.contains_key("provenance"));

        // With matching labels the post-pass DOES write provenance.
        r.extra.insert(
            "labels".to_string(),
            serde_json::json!({"app.kubernetes.io/managed-by": "argocd"}),
        );
        let mut resources = vec![r];
        apply_provenance(
            &mut resources,
            &["app.kubernetes.io/managed-by".to_string()],
            &[],
        );
        let prov = resources[0]
            .extra
            .get("provenance")
            .expect("provenance written");
        assert_eq!(prov["source_kind"].as_str(), Some("argocd"));
    }
}
