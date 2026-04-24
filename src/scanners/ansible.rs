use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

/// Scanner for Ansible assets: playbooks, inventories, and roles.
///
/// Scope is deliberately narrow for the first iteration — we detect the
/// three artefact types that dominate real Ansible-driven repos:
///
/// - `ansible_playbook`  — a top-level YAML file whose root node is a list
///   of plays (each play is a mapping with `hosts` or `import_playbook`).
/// - `ansible_inventory` — an `inventory.yml` / `inventory.yaml` / `hosts`
///   file, or any YAML under `inventories/`.
/// - `ansible_role`      — a directory containing `tasks/main.yml` (the
///   canonical indicator of an Ansible role).
///
/// Jinja2 templates (`.j2`) and individual role tasks are *not* first-class
/// resources yet; the role resource carries a summary of what's inside.
pub struct AnsibleScanner;

#[async_trait]
impl Scanner for AnsibleScanner {
    fn id(&self) -> &str {
        "ansible"
    }
    fn name(&self) -> &str {
        "Ansible Scanner"
    }
    fn description(&self) -> &str {
        "Detects Ansible playbooks, inventories, and roles"
    }
    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();
        let mut claimed: HashSet<std::path::PathBuf> = HashSet::new();

        // Roles first — their presence consumes the `tasks/main.yml` files
        // so they aren't re-reported as orphan playbooks.
        for role in detect_roles(context)? {
            if let Some(task_main) = role.task_main_path.clone() {
                claimed.insert(task_main);
            }
            resources.push(role.into_resource());
        }

        // Inventories next — they're path-conventional, so detection is
        // a filename match, not a content heuristic.
        for path in find_inventories(context)? {
            if context.is_excluded(&path) || claimed.contains(&path) {
                continue;
            }
            if let Some(r) = make_inventory(&path, &context.dir) {
                claimed.insert(path);
                resources.push(r);
            }
        }

        // Playbooks last. We walk every YAML file outside excluded dirs
        // and ask "does this look like a playbook?" — the content check
        // below avoids false positives on docker-compose / k8s manifests.
        for path in find_yaml_files(context)? {
            if context.is_excluded(&path) || claimed.contains(&path) {
                continue;
            }
            if let Some(r) = maybe_playbook(&path, &context.dir) {
                resources.push(r);
            }
        }

        // RFC 007 v0.1: also surface .j2 templates as first-class resources.
        // Rendering runs in the registry post-pass so it can read Compositfile
        // `scan.ansible.extra_vars` without threading ScanSettings through here.
        for path in find_templates(context)? {
            if context.is_excluded(&path) {
                continue;
            }
            if let Some(r) = make_template(&path, &context.dir) {
                resources.push(r);
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
            resolution: None,
        })
    }
}

// ── Role detection ────────────────────────────────────────────────────

struct DetectedRole {
    dir: std::path::PathBuf,
    base_dir: std::path::PathBuf,
    task_main_path: Option<std::path::PathBuf>,
    has_handlers: bool,
    has_vars: bool,
    has_templates: bool,
    template_count: usize,
}

impl DetectedRole {
    fn into_resource(self) -> Resource {
        let rel = self
            .dir
            .strip_prefix(&self.base_dir)
            .unwrap_or(&self.dir)
            .to_string_lossy()
            .to_string();
        let name = self
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "role".to_string());

        let mut extra = HashMap::new();
        extra.insert(
            "has_handlers".to_string(),
            serde_json::Value::Bool(self.has_handlers),
        );
        extra.insert(
            "has_vars".to_string(),
            serde_json::Value::Bool(self.has_vars),
        );
        extra.insert(
            "has_templates".to_string(),
            serde_json::Value::Bool(self.has_templates),
        );
        extra.insert(
            "template_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(self.template_count)),
        );

        Resource {
            resource_type: "ansible_role".to_string(),
            name: Some(name),
            path: Some(format!("./{}", rel)),
            provider: None,
            created: None,
            created_by: None,
            detected_by: "ansible".to_string(),
            estimated_cost: None,
            extra,
        }
    }
}

fn detect_roles(context: &ScanContext) -> Result<Vec<DetectedRole>> {
    let mut roles = Vec::new();
    // tasks/main.yml is the unambiguous role marker.
    for pattern in &["**/tasks/main.yml", "**/tasks/main.yaml"] {
        let full = context.dir.join(pattern);
        for path in glob(&full.to_string_lossy())?.flatten() {
            if context.is_excluded(&path) {
                continue;
            }
            // tasks/ dir's parent is the role dir.
            let tasks_dir = match path.parent() {
                Some(p) => p.to_path_buf(),
                None => continue,
            };
            let role_dir = match tasks_dir.parent() {
                Some(p) => p.to_path_buf(),
                None => continue,
            };
            // Skip if role_dir is directly the scan root — that would
            // mean the repo itself is structured as a role, which is
            // unusual and probably a false positive.
            if role_dir == context.dir {
                continue;
            }

            let templates_dir = role_dir.join("templates");
            let template_count = if templates_dir.is_dir() {
                count_files(&templates_dir, ".j2")
            } else {
                0
            };

            roles.push(DetectedRole {
                task_main_path: Some(path.clone()),
                has_handlers: role_dir.join("handlers").is_dir(),
                has_vars: role_dir.join("vars").is_dir() || role_dir.join("defaults").is_dir(),
                has_templates: templates_dir.is_dir(),
                template_count,
                dir: role_dir,
                base_dir: context.dir.clone(),
            });
        }
    }
    Ok(roles)
}

fn count_files(dir: &Path, suffix: &str) -> usize {
    let Ok(iter) = std::fs::read_dir(dir) else {
        return 0;
    };
    iter.filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .to_lowercase()
                .ends_with(suffix)
        })
        .count()
}

// ── Inventory detection ──────────────────────────────────────────────

fn find_inventories(context: &ScanContext) -> Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    let patterns = &[
        "**/inventory.yml",
        "**/inventory.yaml",
        "**/inventory.ini",
        "**/inventories/*.yml",
        "**/inventories/*.yaml",
        "**/inventories/*.ini",
        "**/hosts.yml",
        "**/hosts.yaml",
        "**/hosts.ini",
    ];
    for pattern in patterns {
        let full = context.dir.join(pattern);
        for path in glob(&full.to_string_lossy())?.flatten() {
            out.push(path);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn make_inventory(path: &Path, base_dir: &Path) -> Option<Resource> {
    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let mut extra = HashMap::new();

    // For YAML inventories, count top-level groups. For INI, we skip the
    // content introspection (ini parser would be extra dependency).
    if path.extension().and_then(|e| e.to_str()) != Some("ini") {
        let Ok(content) = std::fs::read_to_string(path) else {
            return None;
        };
        if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
            if let Some(map) = val.as_mapping() {
                let groups: Vec<String> = map
                    .keys()
                    .filter_map(|k| k.as_str().map(str::to_string))
                    .collect();
                extra.insert(
                    "groups".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(groups.len())),
                );
                extra.insert(
                    "group_names".to_string(),
                    serde_json::Value::Array(
                        groups.into_iter().map(serde_json::Value::String).collect(),
                    ),
                );
            }
        }
    }

    Some(Resource {
        resource_type: "ansible_inventory".to_string(),
        name: path.file_name().map(|n| n.to_string_lossy().to_string()),
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "ansible".to_string(),
        estimated_cost: None,
        extra,
    })
}

// ── Template detection (RFC 007) ─────────────────────────────────────

fn find_templates(context: &ScanContext) -> Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    for pattern in &["**/templates/**/*.j2", "**/templates/*.j2"] {
        let full = context.dir.join(pattern);
        for path in glob(&full.to_string_lossy())?.flatten() {
            out.push(path);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn make_template(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;
    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let name = path.file_name().map(|n| n.to_string_lossy().to_string());
    // Heuristic owning role: the directory containing `templates/`.
    let parent_role = path
        .ancestors()
        .find(|p| p.file_name().map(|n| n == "templates").unwrap_or(false))
        .and_then(|t| t.parent())
        .and_then(|r| r.file_name())
        .map(|n| n.to_string_lossy().to_string());

    // RFC 007 §Open question 1: ansible-vault encrypted files carry a
    // magic header. We never attempt to render them — decryption is a
    // deliberate user action that needs a key, not something composit
    // should reach for. Flag the file so diff can surface a warning
    // without failing the rest of the scan.
    let is_vault = content.starts_with("$ANSIBLE_VAULT;");

    let mut extra = HashMap::new();
    if let Some(role) = parent_role {
        extra.insert("parent_role".to_string(), serde_json::Value::String(role));
    }
    if is_vault {
        extra.insert("vault_encrypted".to_string(), serde_json::Value::Bool(true));
    } else {
        // Stash the raw source so the registry-side renderer can pick it
        // up without re-reading the file. Not serialised in the final
        // report — stripped after rendering.
        extra.insert(
            "template_source".to_string(),
            serde_json::Value::String(content),
        );
    }
    // Empty renderings field is always present so diff/HTML have a stable
    // shape even when rendering didn't run (no extra_vars).
    extra.insert(
        "renderings".to_string(),
        serde_json::Value::Array(Vec::new()),
    );

    Some(Resource {
        resource_type: "ansible_template".to_string(),
        name,
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "ansible".to_string(),
        estimated_cost: None,
        extra,
    })
}

// ── Playbook detection ───────────────────────────────────────────────

fn find_yaml_files(context: &ScanContext) -> Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    for pattern in &["**/*.yml", "**/*.yaml"] {
        let full = context.dir.join(pattern);
        for path in glob(&full.to_string_lossy())?.flatten() {
            out.push(path);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Return a playbook resource if the YAML looks like a playbook.
/// Heuristic: root must be a sequence, and at least one top-level entry
/// must carry a `hosts` or `import_playbook` key — those are the two
/// recognised Ansible play forms.
fn maybe_playbook(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    let seq = yaml.as_sequence()?;

    let mut plays_with_hosts = 0usize;
    let mut imports = 0usize;
    let mut task_counts = Vec::<usize>::new();
    let mut role_refs = Vec::<String>::new();

    for play in seq {
        let map = play.as_mapping()?;
        let has_hosts = map.iter().any(|(k, _)| k.as_str() == Some("hosts"));
        let has_import = map
            .iter()
            .any(|(k, _)| k.as_str() == Some("import_playbook"));
        if !(has_hosts || has_import) {
            return None;
        }
        if has_hosts {
            plays_with_hosts += 1;
        }
        if has_import {
            imports += 1;
        }

        if let Some(tasks) = map
            .iter()
            .find(|(k, _)| k.as_str() == Some("tasks"))
            .and_then(|(_, v)| v.as_sequence())
        {
            task_counts.push(tasks.len());
        }
        if let Some(roles) = map
            .iter()
            .find(|(k, _)| k.as_str() == Some("roles"))
            .and_then(|(_, v)| v.as_sequence())
        {
            for r in roles {
                if let Some(name) = r.as_str() {
                    role_refs.push(name.to_string());
                } else if let Some(m) = r.as_mapping() {
                    if let Some(n) = m.iter().find(|(k, _)| k.as_str() == Some("role")) {
                        if let Some(s) = n.1.as_str() {
                            role_refs.push(s.to_string());
                        }
                    }
                }
            }
        }
    }

    if plays_with_hosts + imports == 0 {
        return None;
    }

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let mut extra = HashMap::new();
    extra.insert(
        "plays".to_string(),
        serde_json::Value::Number(serde_json::Number::from(plays_with_hosts)),
    );
    if imports > 0 {
        extra.insert(
            "imports".to_string(),
            serde_json::Value::Number(serde_json::Number::from(imports)),
        );
    }
    let total_tasks: usize = task_counts.iter().sum();
    if total_tasks > 0 {
        extra.insert(
            "tasks".to_string(),
            serde_json::Value::Number(serde_json::Number::from(total_tasks)),
        );
    }
    if !role_refs.is_empty() {
        extra.insert(
            "roles".to_string(),
            serde_json::Value::Array(
                role_refs
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }

    Some(Resource {
        resource_type: "ansible_playbook".to_string(),
        name: path.file_stem().map(|s| s.to_string_lossy().to_string()),
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "ansible".to_string(),
        estimated_cost: None,
        extra,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(dir: &Path, rel: &str, content: &str) {
        let full = dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full, content).unwrap();
    }

    #[test]
    fn playbook_with_hosts_detected() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "site.yml",
            "- hosts: all\n  tasks:\n    - name: echo\n      debug: msg=hi\n",
        );
        let r = maybe_playbook(&dir.path().join("site.yml"), dir.path()).unwrap();
        assert_eq!(r.resource_type, "ansible_playbook");
        assert_eq!(r.extra.get("plays").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(r.extra.get("tasks").and_then(|v| v.as_u64()), Some(1));
    }

    #[test]
    fn playbook_with_import_detected() {
        let dir = tempdir().unwrap();
        write(dir.path(), "main.yml", "- import_playbook: sub.yml\n");
        let r = maybe_playbook(&dir.path().join("main.yml"), dir.path()).unwrap();
        assert_eq!(r.resource_type, "ansible_playbook");
    }

    #[test]
    fn non_playbook_yaml_rejected() {
        let dir = tempdir().unwrap();
        // Looks like k8s — has a root mapping (not a sequence), so the
        // sequence check fails and we return None.
        write(
            dir.path(),
            "deployment.yml",
            "apiVersion: apps/v1\nkind: Deployment\n",
        );
        assert!(maybe_playbook(&dir.path().join("deployment.yml"), dir.path()).is_none());

        // Sequence but entries lack hosts/import_playbook — docker-compose
        // is a mapping so it never gets past as_sequence(), but a random
        // list of env files is a sequence and must still be rejected.
        write(dir.path(), "list.yml", "- a\n- b\n");
        assert!(maybe_playbook(&dir.path().join("list.yml"), dir.path()).is_none());
    }

    #[test]
    fn inventory_yaml_counts_groups() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "inventory.yml",
            "all:\n  children:\n    web:\n      hosts:\n        h1:\n    db:\n      hosts:\n        h2:\n",
        );
        let r = make_inventory(&dir.path().join("inventory.yml"), dir.path()).unwrap();
        assert_eq!(r.resource_type, "ansible_inventory");
        assert_eq!(r.extra.get("groups").and_then(|v| v.as_u64()), Some(1));
        let names: Vec<&str> = r
            .extra
            .get("group_names")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(names, vec!["all"]);
    }
}
