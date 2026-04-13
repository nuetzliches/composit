use std::collections::HashSet;

use super::types::{Provider, ProviderStatus, Resource};

/// Deduplicate providers by endpoint URL, preferring reachable status.
pub fn dedup_providers(providers: Vec<Provider>) -> Vec<Provider> {
    let mut seen: std::collections::HashMap<String, Provider> = std::collections::HashMap::new();

    for p in providers {
        let key = p.endpoint.clone();
        if let Some(existing) = seen.get(&key) {
            // Prefer reachable over unknown/unreachable
            let should_replace = matches!(p.status, ProviderStatus::Reachable)
                && !matches!(existing.status, ProviderStatus::Reachable);
            if should_replace {
                seen.insert(key, p);
            }
        } else {
            seen.insert(key, p);
        }
    }

    seen.into_values().collect()
}

/// Deduplicate resources by (type, name, provider) or (type, path).
pub fn dedup_resources(resources: Vec<Resource>) -> Vec<Resource> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut result = Vec::new();

    for r in resources {
        let key = if let Some(path) = &r.path {
            format!("{}:{}", r.resource_type, path)
        } else if let (Some(name), Some(provider)) = (&r.name, &r.provider) {
            format!("{}:{}:{}", r.resource_type, name, provider)
        } else if let Some(name) = &r.name {
            format!("{}:{}", r.resource_type, name)
        } else {
            // No dedup key possible, keep it
            result.push(r);
            continue;
        };

        if seen.insert(key) {
            result.push(r);
        }
    }

    result
}
