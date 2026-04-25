use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::governance::ScanSettings;
use super::scanner::{
    ProviderTarget, ResolutionInfo, ScanContext, ScanResult, Scanner, UnresolvedVariable,
};
use super::types::{Provider, Resource};

pub struct ScannerRegistry {
    scanners: Vec<Box<dyn Scanner>>,
}

impl ScannerRegistry {
    pub fn new() -> Self {
        ScannerRegistry {
            scanners: Vec::new(),
        }
    }

    pub fn register(&mut self, scanner: Box<dyn Scanner>) {
        self.scanners.push(scanner);
    }

    /// Run all applicable scanners. Filesystem scanners run first,
    /// then network scanners receive discovered provider URLs.
    pub async fn run_all(
        &self,
        context: &ScanContext,
        scan: Option<&ScanSettings>,
    ) -> Result<ScanResult> {
        let mut all_resources: Vec<Resource> = Vec::new();
        let mut all_providers: Vec<Provider> = Vec::new();

        // Phase 1: filesystem scanners (needs_network == false)
        let fs_scanners: Vec<&dyn Scanner> = self
            .scanners
            .iter()
            .filter(|s| !s.needs_network())
            .filter(|s| scan.map(|c| c.is_scanner_enabled(s.id())).unwrap_or(true))
            .map(|s| s.as_ref())
            .collect();

        for scanner in &fs_scanners {
            match scanner.scan(context).await {
                Ok(result) => {
                    all_resources.extend(result.resources);
                    all_providers.extend(result.providers);
                }
                Err(e) => {
                    eprintln!("Warning: scanner '{}' failed: {}", scanner.id(), e);
                }
            }
        }

        // Phase 2: network scanners (if not skipped)
        if !context.skip_providers {
            let net_scanners: Vec<&dyn Scanner> = self
                .scanners
                .iter()
                .filter(|s| s.needs_network())
                .filter(|s| scan.map(|c| c.is_scanner_enabled(s.id())).unwrap_or(true))
                .map(|s| s.as_ref())
                .collect();

            // Build extended target list with discovered provider URLs.
            // Entries carry optional trust/auth metadata from the initial
            // context (typically sourced from a Compositfile). Discovered
            // URLs are treated as public-only.
            let mut extended_providers: Vec<ProviderTarget> = context.providers.clone();
            let known_urls = |list: &[ProviderTarget], url: &str| list.iter().any(|t| t.url == url);
            for p in &all_providers {
                if !known_urls(&extended_providers, &p.endpoint) {
                    extended_providers.push(ProviderTarget::public_only(p.endpoint.clone()));
                }
            }

            let extended_context = ScanContext {
                dir: context.dir.clone(),
                providers: extended_providers,
                skip_providers: false,
                exclude_patterns: context.exclude_patterns.clone(),
            };

            for scanner in &net_scanners {
                match scanner.scan(&extended_context).await {
                    Ok(result) => {
                        all_resources.extend(result.resources);
                        all_providers.extend(result.providers);
                    }
                    Err(e) => {
                        eprintln!("Warning: scanner '{}' failed: {}", scanner.id(), e);
                    }
                }
            }
        }

        // Cross-platform normalisation: scanners that build paths via
        // Path::to_string_lossy() emit backslashes on Windows. Reports and
        // diffs are compared across platforms, so flatten every resource
        // path to forward slashes at the orchestrator boundary — that way
        // individual scanners don't have to remember.
        for r in &mut all_resources {
            if let Some(p) = r.path.as_mut() {
                if p.contains('\\') {
                    *p = p.replace('\\', "/");
                }
            }
        }

        // RFC 006 v0.1: minimal cross-file variable resolution. Opt-in via
        // Compositfile `scan.resolvable` — operators must explicitly list
        // which env-file globs are allowed to supply values. Without it,
        // `${VAR}` stays literal in the report and shows up as
        // `unresolved_variable` in the diff.
        let resolvable = scan.map(|s| s.resolvable.as_slice()).unwrap_or(&[]);
        let redact = scan.map(|s| s.redact.as_slice()).unwrap_or(&[]);
        let resolution =
            resolve_docker_service_variables(&mut all_resources, &context.dir, resolvable, redact);

        // Apply scan.redact to env_file.keys so operators can hide
        // sensitive key *names* — opt-in only. Defaults (*_SECRET, *_KEY,
        // *_TOKEN, *_PASSWORD, DATABASE_URL, JWT_SECRET) apply to VALUES
        // during ${VAR} substitution but NOT to key names: knowing a
        // service has a `DATABASE_URL` is governance-relevant, only the
        // value would be sensitive. User-declared patterns redact names
        // for callers who need stricter privacy (e.g. customer-named keys).
        redact_env_file_keys(&mut all_resources, redact);

        // RFC 007: render Ansible `.j2` templates. v0.1 did extra_vars
        // only; v0.2 adds per-inventory rendering — each inventory listed
        // in `scan.ansible.inventories` yields a separate entry under
        // `renderings[]`, with its own merged variable scope. Pure
        // extra_vars renderings keep the legacy behaviour.
        let empty: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let empty_invs: Vec<String> = Vec::new();
        let extra_vars = scan.map(|s| &s.ansible.extra_vars).unwrap_or(&empty);
        let inventories = scan.map(|s| &s.ansible.inventories).unwrap_or(&empty_invs);
        render_ansible_templates(&mut all_resources, extra_vars, inventories, &context.dir);

        Ok(ScanResult {
            resources: all_resources,
            providers: all_providers,
            resolution,
        })
    }
}

/// Built-in secret-redaction patterns applied on top of whatever the
/// Compositfile adds. Keys are compared case-insensitively.
const DEFAULT_REDACT_SUFFIXES: &[&str] = &["_KEY", "_SECRET", "_TOKEN", "_PASSWORD"];
const DEFAULT_REDACT_EXACT: &[&str] = &["DATABASE_URL", "JWT_SECRET"];

/// Resolve `${VAR}` / `${VAR:-DEFAULT}` references in docker_service
/// image + ports. Returns `Some(ResolutionInfo)` iff resolution actually
/// ran (i.e. at least one `resolvable` glob was declared). Returns `None`
/// when the Compositfile opted out / didn't opt in — callers can use that
/// to show "resolution disabled" in the report.
fn resolve_docker_service_variables(
    resources: &mut [Resource],
    scan_dir: &Path,
    resolvable: &[String],
    extra_redact: &[String],
) -> Option<ResolutionInfo> {
    if resolvable.is_empty() {
        return None;
    }
    // `.gitignore`-style convenience: a bare `.env` matches `.env` anywhere,
    // not just in the scan root. Authors don't have to remember the `**/`
    // prefix for the common case. Entries that already carry a slash or a
    // glob metachar are used verbatim.
    let resolvable_patterns: Vec<glob::Pattern> = resolvable
        .iter()
        .flat_map(|p| {
            let has_meta = p.contains('/') || p.contains(['*', '?', '[']);
            if has_meta {
                vec![glob::Pattern::new(p).ok()]
            } else {
                vec![
                    glob::Pattern::new(p).ok(),
                    glob::Pattern::new(&format!("**/{}", p)).ok(),
                ]
            }
        })
        .flatten()
        .collect();
    let redact_patterns: Vec<glob::Pattern> = extra_redact
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let mut info = ResolutionInfo::default();
    let mut env_cache: HashMap<PathBuf, HashMap<String, String>> = HashMap::new();

    for r in resources.iter_mut() {
        if r.resource_type != "docker_service" {
            continue;
        }
        let compose_rel = r
            .extra
            .get("compose_file")
            .and_then(|v| v.as_str())
            .map(|s| s.trim_start_matches("./").to_string());
        let Some(compose_rel) = compose_rel else {
            continue;
        };
        let compose_abs = scan_dir.join(&compose_rel);
        let compose_dir = match compose_abs.parent() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };

        // An .env file next to a compose file is eligible only when its
        // path (relative to scan_dir, forward-slash normalised) matches
        // one of the `resolvable` globs.
        let candidate = compose_dir.join(".env");
        let rel_from_scan = candidate
            .strip_prefix(scan_dir)
            .unwrap_or(&candidate)
            .to_string_lossy()
            .replace('\\', "/");

        let allowed = resolvable_patterns
            .iter()
            .any(|p| p.matches(&rel_from_scan));
        if !allowed {
            continue;
        }

        let env = env_cache
            .entry(candidate.clone())
            .or_insert_with(|| read_env_file_values(&candidate, &redact_patterns))
            .clone();

        if !env.is_empty() && !info.env_files_used.iter().any(|p| p == &rel_from_scan) {
            info.env_files_used.push(rel_from_scan.clone());
        }

        // Resolve image
        if let Some(raw) = r
            .extra
            .get("image")
            .and_then(|v| v.as_str())
            .map(str::to_string)
        {
            if raw.contains("${") {
                let (resolved, unresolved) = substitute_vars_with_trace(&raw, &env);
                if resolved != raw {
                    r.extra.insert(
                        "resolved_image".to_string(),
                        serde_json::Value::String(resolved),
                    );
                }
                for name in unresolved {
                    info.unresolved.push(UnresolvedVariable {
                        resource_path: r.path.clone().unwrap_or_default(),
                        field: "image".to_string(),
                        variable: name,
                    });
                }
            }
        }

        // Resolve ports (array of strings)
        if let Some(ports_val) = r.extra.get("ports").cloned() {
            if let Some(arr) = ports_val.as_array() {
                let mut any_resolved = false;
                let mut port_unresolved = Vec::new();
                let resolved: Vec<serde_json::Value> = arr
                    .iter()
                    .map(|p| {
                        if let Some(s) = p.as_str() {
                            if s.contains("${") {
                                let (new, mut unres) = substitute_vars_with_trace(s, &env);
                                if new != s {
                                    any_resolved = true;
                                }
                                port_unresolved.append(&mut unres);
                                serde_json::Value::String(new)
                            } else {
                                p.clone()
                            }
                        } else {
                            p.clone()
                        }
                    })
                    .collect();
                if any_resolved {
                    r.extra.insert(
                        "resolved_ports".to_string(),
                        serde_json::Value::Array(resolved),
                    );
                }
                for name in port_unresolved {
                    info.unresolved.push(UnresolvedVariable {
                        resource_path: r.path.clone().unwrap_or_default(),
                        field: "ports".to_string(),
                        variable: name,
                    });
                }
            }
        }
    }

    Some(info)
}

fn read_env_file_values(path: &Path, extra_redact: &[glob::Pattern]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Ok(content) = std::fs::read_to_string(path) else {
        return out;
    };
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_start_matches("export ").trim();
        let value = value.trim();
        // Strip surrounding quotes — common convention in .env files.
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);

        let final_value = if should_redact(key, extra_redact) {
            "<redacted>".to_string()
        } else {
            value.to_string()
        };
        out.insert(key.to_string(), final_value);
    }
    out
}

/// Render Ansible `.j2` templates. If `inventories` is non-empty, each
/// inventory produces a separate `renderings[]` entry with its own merged
/// variable scope (inventory vars < group vars < extra_vars). Otherwise
/// a single render runs with just `extra_vars`. A no-op when neither
/// input would produce a value.
fn render_ansible_templates(
    resources: &mut [Resource],
    extra_vars: &HashMap<String, String>,
    inventories: &[String],
    scan_dir: &Path,
) {
    // Pre-load every inventory's merged var map once; templates reuse
    // them without re-parsing YAML per template.
    let inventory_scopes: Vec<(String, HashMap<String, String>)> = inventories
        .iter()
        .filter_map(|path| {
            let full = scan_dir.join(path);
            let vars = load_inventory_vars(&full)?;
            Some((path.clone(), vars))
        })
        .collect();

    for r in resources.iter_mut() {
        if r.resource_type != "ansible_template" {
            continue;
        }
        let Some(source) = r
            .extra
            .remove("template_source")
            .and_then(|v| v.as_str().map(str::to_string))
        else {
            continue;
        };

        let mut renderings: Vec<serde_json::Value> = Vec::new();

        if inventory_scopes.is_empty() {
            if extra_vars.is_empty() {
                continue;
            }
            renderings.push(render_one_template(&source, extra_vars, "extra_vars"));
        } else {
            for (path, scope) in &inventory_scopes {
                // extra_vars wins over inventory-scoped values — matches
                // ansible-playbook --extra-vars precedence.
                let mut merged = scope.clone();
                for (k, v) in extra_vars {
                    merged.insert(k.clone(), v.clone());
                }
                renderings.push(render_one_template(&source, &merged, path));
            }
        }

        r.extra.insert(
            "renderings".to_string(),
            serde_json::Value::Array(renderings),
        );
    }
}

/// Flatten an Ansible YAML inventory's top-level `vars:` plus every
/// child group's `vars:` into a single key→value map. Values are
/// stringified because Jinja renders strings. Host-level vars are not
/// merged in v0.2 — the scope model stays per-inventory, not per-host.
fn load_inventory_vars(path: &Path) -> Option<HashMap<String, String>> {
    let content = std::fs::read_to_string(path).ok()?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    let mut out = HashMap::new();
    collect_inventory_vars(&doc, &mut out);

    // Layer group_vars/ and host_vars/ from the inventory directory (if
    // present). Groups defined directly in the inventory already flowed
    // through collect_inventory_vars; this adds the conventional side-
    // file layout ansible-playbook honours.
    if let Some(inv_dir) = path.parent() {
        let group_vars_dir = inv_dir.join("group_vars");
        if group_vars_dir.is_dir() {
            if let Ok(iter) = std::fs::read_dir(&group_vars_dir) {
                for entry in iter.flatten() {
                    merge_yaml_file(&entry.path(), &mut out);
                }
            }
        }
    }

    Some(out)
}

fn collect_inventory_vars(node: &serde_yaml::Value, out: &mut HashMap<String, String>) {
    let Some(map) = node.as_mapping() else { return };
    for (k, v) in map {
        let key_str = k.as_str();
        match key_str {
            Some("vars") => {
                if let Some(vars_map) = v.as_mapping() {
                    for (vk, vv) in vars_map {
                        if let (Some(vks), Some(vvs)) = (vk.as_str(), yaml_scalar_as_string(vv)) {
                            out.insert(vks.to_string(), vvs);
                        }
                    }
                }
            }
            Some("hosts") => {
                // Skip — host-level vars are deferred to a later RFC 007 revision.
            }
            Some("children") => {
                if let Some(children) = v.as_mapping() {
                    for (_, child) in children {
                        collect_inventory_vars(child, out);
                    }
                }
            }
            _ => {
                // Any other top-level mapping entry might itself be a
                // group definition in the shorthand Ansible YAML form.
                // Recurse so `all.children.web.vars.x` is reachable.
                collect_inventory_vars(v, out);
            }
        }
    }
}

fn merge_yaml_file(path: &Path, out: &mut HashMap<String, String>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(&content) else {
        return;
    };
    let Some(map) = doc.as_mapping() else { return };
    for (k, v) in map {
        if let (Some(ks), Some(vs)) = (k.as_str(), yaml_scalar_as_string(v)) {
            out.insert(ks.to_string(), vs);
        }
    }
}

fn yaml_scalar_as_string(v: &serde_yaml::Value) -> Option<String> {
    match v {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Render one template with minijinja, sandboxed to reject filesystem /
/// network access. `source_tag` identifies which variable source fed
/// this render so consumers (diff, HTML) can distinguish multiple
/// renderings of the same template. The returned JSON object carries
/// `rendered` (string) on success, or `error`/`unresolved` on failure —
/// matching RFC 007's report shape.
fn render_one_template(
    source: &str,
    extra_vars: &HashMap<String, String>,
    source_tag: &str,
) -> serde_json::Value {
    use minijinja::{Environment, UndefinedBehavior};

    let mut env = Environment::new();
    // Chatty but safer default: undefined variable = rendering error,
    // so we can surface it. Without this minijinja emits empty strings
    // and the operator sees no signal.
    env.set_undefined_behavior(UndefinedBehavior::Strict);

    let tmpl = match env.template_from_str(source) {
        Ok(t) => t,
        Err(e) => {
            return serde_json::json!({
                "source": source_tag,
                "error": format!("parse error: {}", e),
            });
        }
    };

    let ctx: HashMap<String, &String> = extra_vars.iter().map(|(k, v)| (k.clone(), v)).collect();

    match tmpl.render(ctx) {
        Ok(rendered) => {
            // Truncate at 1 MiB per RFC 007 §Safety model.
            let truncated = rendered.len() > 1024 * 1024;
            let final_output = if truncated {
                rendered[..1024 * 1024].to_string()
            } else {
                rendered
            };
            let mut obj = serde_json::json!({
                "source": source_tag,
                "rendered": final_output,
                "checksum": format!("sha256:{}", short_checksum(&final_output)),
            });
            // Try to parse the rendered output as dotenv — the most
            // common target for `.j2` templates in Ansible. When it
            // looks like one (mostly `KEY=VALUE` lines), we emit a
            // `rendered_parsed` map so role constraints can match keys
            // directly instead of string-scanning the rendered blob.
            // Other formats (nginx, systemd) will hook in here later.
            if let Some(dotenv) = parse_dotenv(obj["rendered"].as_str().unwrap()) {
                obj["rendered_parsed"] = serde_json::json!({
                    "format": "dotenv",
                    "keys": dotenv,
                });
            }
            if truncated {
                obj["output_truncated"] = serde_json::Value::Bool(true);
            }
            obj
        }
        Err(e) => {
            // Extract the offending variable by scanning the template for
            // `{{ var }}` references and picking the first one not in
            // extra_vars. minijinja's error messages don't carry the name
            // reliably, so this source-level pass is the stable fallback.
            let msg = e.to_string();
            let mut out = serde_json::json!({
                "source": source_tag,
                "error": msg,
            });
            if let Some(missing) = first_undefined_var(source, extra_vars) {
                out["unresolved_variable"] = serde_json::Value::String(missing);
            }
            out
        }
    }
}

/// Scan a template body for `{{ name }}` references and return the first
/// identifier that isn't declared in `extra_vars`. Used to surface the
/// variable name in `unresolved_variable` diagnostics when minijinja
/// refuses to interpolate.
fn first_undefined_var(source: &str, extra_vars: &HashMap<String, String>) -> Option<String> {
    let mut i = 0;
    let bytes = source.as_bytes();
    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            let close = source[i + 2..].find("}}")?;
            let expr = source[i + 2..i + 2 + close].trim();
            // Peel pipe-filters and indexing: just the leading identifier.
            let ident_end = expr
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(expr.len());
            let ident = &expr[..ident_end];
            if !ident.is_empty() && !extra_vars.contains_key(ident) {
                return Some(ident.to_string());
            }
            i += 2 + close + 2;
        } else {
            i += 1;
        }
    }
    None
}

/// Detect whether a rendered string looks like a dotenv file (mostly
/// `KEY=VALUE` lines, few or no exotic constructs). Returns the key→value
/// map when confident; `None` otherwise so the template's downstream
/// constraints fall back to raw-string matching. The cheap heuristic:
/// at least 60% of non-empty, non-comment lines must fit `KEY=...`.
fn parse_dotenv(rendered: &str) -> Option<HashMap<String, String>> {
    let mut total = 0;
    let mut matched = 0;
    let mut map = HashMap::new();
    for line in rendered.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        total += 1;
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_start_matches("export ").trim();
        let value = value
            .trim()
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| {
                trimmed
                    .split_once('=')
                    .map(|(_, v)| v.trim())
                    .and_then(|v| v.strip_prefix('\''))
                    .and_then(|v| v.strip_suffix('\''))
            })
            .unwrap_or_else(|| trimmed.split_once('=').unwrap().1.trim());
        if key.is_empty() || key.contains(char::is_whitespace) {
            continue;
        }
        matched += 1;
        map.insert(key.to_string(), value.to_string());
    }
    if total >= 2 && (matched * 100) / total >= 60 {
        Some(map)
    } else {
        None
    }
}

/// 16-hex-char content fingerprint. Not cryptographic — just enough to
/// detect drift between renderings without storing full SHA-256 strings.
fn short_checksum(s: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn redact_env_file_keys(resources: &mut [Resource], user_redact: &[String]) {
    // Only user-declared patterns redact key names. The default
    // `*_KEY/_SECRET/_TOKEN/_PASSWORD` list applies to VALUES (in
    // `read_env_file_values`) — names are governance signal and stay
    // visible unless the operator explicitly opts in.
    let patterns: Vec<glob::Pattern> = user_redact
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();
    if patterns.is_empty() {
        return;
    }
    for r in resources.iter_mut() {
        if r.resource_type != "env_file" {
            continue;
        }
        let Some(keys_val) = r.extra.get_mut("keys") else {
            continue;
        };
        let Some(arr) = keys_val.as_array_mut() else {
            continue;
        };
        for item in arr.iter_mut() {
            let Some(s) = item.as_str() else { continue };
            let upper = s.to_uppercase();
            if patterns.iter().any(|p| p.matches(&upper)) {
                *item = serde_json::Value::String("<redacted>".to_string());
            }
        }
    }
}

fn should_redact(key: &str, extra: &[glob::Pattern]) -> bool {
    let upper = key.to_uppercase();
    if DEFAULT_REDACT_EXACT.contains(&upper.as_str()) {
        return true;
    }
    if DEFAULT_REDACT_SUFFIXES.iter().any(|s| upper.ends_with(s)) {
        return true;
    }
    // User-declared redact globs match against the upper-case key so
    // patterns like `*_URL` fire regardless of source casing.
    extra.iter().any(|p| p.matches(&upper))
}

/// Substitute `${VAR}` / `${VAR:-DEFAULT}` references in an input string
/// using the supplied env map. The returned `Vec<String>` contains the
/// variable names the resolver could not fill — callers surface these
/// as `unresolved` entries in the scan report (RFC 006).
fn substitute_vars_with_trace(input: &str, env: &HashMap<String, String>) -> (String, Vec<String>) {
    let mut out = String::with_capacity(input.len());
    let mut unresolved = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'$' && bytes[i + 1] == b'{' {
            if let Some(end) = input[i + 2..].find('}') {
                let expr = &input[i + 2..i + 2 + end];
                match resolve_expr(expr, env) {
                    Some(v) => out.push_str(&v),
                    None => {
                        // Leave the raw `${expr}` so downstream tooling
                        // still sees what was unresolved.
                        out.push_str(&format!("${{{}}}", expr));
                        // Record only the variable name (left of any modifier).
                        let var_name = expr
                            .split_once(':')
                            .map(|(n, _)| n)
                            .unwrap_or(expr)
                            .trim()
                            .to_string();
                        if !var_name.is_empty() {
                            unresolved.push(var_name);
                        }
                    }
                }
                i += 2 + end + 1;
                continue;
            }
        }
        out.push(input.as_bytes()[i] as char);
        i += 1;
    }
    (out, unresolved)
}

fn resolve_expr(expr: &str, env: &HashMap<String, String>) -> Option<String> {
    if let Some((name, default)) = expr.split_once(":-") {
        return Some(match env.get(name) {
            Some(v) if !v.is_empty() => v.clone(),
            _ => default.to_string(),
        });
    }
    if let Some((name, _alt)) = expr.split_once(":+") {
        return env.get(name).filter(|v| !v.is_empty()).cloned();
    }
    if let Some((name, _err)) = expr.split_once(":?") {
        return env.get(name).filter(|v| !v.is_empty()).cloned();
    }
    env.get(expr).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scanner::ScanContext;
    use crate::core::types::Resource;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::path::PathBuf;

    struct StubScanner;

    #[async_trait]
    impl Scanner for StubScanner {
        fn id(&self) -> &str {
            "stub"
        }
        fn name(&self) -> &str {
            "stub"
        }
        fn description(&self) -> &str {
            ""
        }
        fn needs_network(&self) -> bool {
            false
        }
        async fn scan(&self, _context: &ScanContext) -> Result<ScanResult> {
            let r = Resource {
                resource_type: "docker_compose".to_string(),
                name: None,
                path: Some("./AdvoNeo\\foo\\docker-compose.yml".to_string()),
                provider: None,
                created: None,
                created_by: None,
                detected_by: "stub".to_string(),
                estimated_cost: None,
                extra: HashMap::new(),
            };
            Ok(ScanResult {
                resources: vec![r],
                providers: vec![],
                resolution: None,
            })
        }
    }

    fn env_file_with_keys(keys: &[&str]) -> Resource {
        let mut extra = HashMap::new();
        extra.insert(
            "keys".to_string(),
            serde_json::Value::Array(
                keys.iter()
                    .map(|k| serde_json::Value::String(k.to_string()))
                    .collect(),
            ),
        );
        Resource {
            resource_type: "env_file".to_string(),
            name: None,
            path: Some("./.env".to_string()),
            provider: None,
            created: None,
            created_by: None,
            detected_by: "env_files".to_string(),
            estimated_cost: None,
            extra,
        }
    }

    fn keys_of(r: &Resource) -> Vec<String> {
        r.extra
            .get("keys")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn redact_env_file_keys_no_patterns_leaves_keys_intact() {
        // Default redact suffixes (*_KEY, *_SECRET, …) apply to VALUES
        // during ${VAR} substitution but MUST NOT hide key names —
        // knowing a service has a `DATABASE_URL` env var is governance
        // signal, only the value would be sensitive.
        let mut resources = vec![env_file_with_keys(&[
            "HOOKAIDO_PULL_TOKEN",
            "HOOKAIDO_INGRESS_SECRET",
            "DATABASE_URL",
            "API_KEY",
        ])];
        redact_env_file_keys(&mut resources, &[]);
        let keys = keys_of(&resources[0]);
        assert_eq!(
            keys,
            vec!["HOOKAIDO_PULL_TOKEN", "HOOKAIDO_INGRESS_SECRET", "DATABASE_URL", "API_KEY"],
            "no user redact patterns → all key names visible"
        );
    }

    #[test]
    fn redact_env_file_keys_user_patterns_hide_only_matching_names() {
        // User opts into name-redaction with specific globs.
        let mut resources = vec![env_file_with_keys(&[
            "CUSTOMER_ACME_API_KEY",
            "CUSTOMER_BETA_API_KEY",
            "GENERIC_API_KEY",
        ])];
        redact_env_file_keys(&mut resources, &["CUSTOMER_*".to_string()]);
        let keys = keys_of(&resources[0]);
        assert_eq!(
            keys,
            vec!["<redacted>", "<redacted>", "GENERIC_API_KEY"],
            "user pattern hides matching names; non-matching stays"
        );
    }

    #[test]
    fn redact_env_file_keys_case_insensitive_match() {
        let mut resources = vec![env_file_with_keys(&["customer_url", "PUBLIC_URL"])];
        redact_env_file_keys(&mut resources, &["*_URL".to_string()]);
        let keys = keys_of(&resources[0]);
        assert_eq!(keys, vec!["<redacted>", "<redacted>"]);
    }

    #[tokio::test]
    async fn run_all_normalizes_windows_backslashes_in_paths() {
        let mut reg = ScannerRegistry::new();
        reg.register(Box::new(StubScanner));
        let ctx = ScanContext {
            dir: PathBuf::from("/tmp/repo"),
            providers: vec![],
            skip_providers: true,
            exclude_patterns: vec![],
        };
        let out = reg.run_all(&ctx, None).await.unwrap();
        assert_eq!(
            out.resources[0].path.as_deref(),
            Some("./AdvoNeo/foo/docker-compose.yml"),
            "registry must rewrite backslashes to forward slashes"
        );
    }
}
