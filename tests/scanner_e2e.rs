//! End-to-end tests that invoke the `composit scan` binary against fixture
//! directories and assert on the written report.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn cargo_bin() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    // current_exe is something like target/debug/deps/scanner_e2e-<hash>
    // walk up to target/<profile>/
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("composit")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn copy_fixture(name: &str, dest: &Path) {
    let src = fixtures_dir().join(name);
    copy_dir_all(&src, dest).unwrap_or_else(|e| panic!("copy fixture {name}: {e}"));
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

fn run_scan(dir: &Path) -> std::process::Output {
    Command::new(cargo_bin())
        .args(["scan", "--dir"])
        .arg(dir)
        .args(["--no-providers", "--output", "json"])
        .output()
        .expect("failed to execute composit")
}

fn run_scan_yaml(dir: &Path) -> std::process::Output {
    Command::new(cargo_bin())
        .args(["scan", "--dir"])
        .arg(dir)
        .args(["--no-providers", "--quiet"])
        .output()
        .expect("failed to execute composit")
}

fn run_diff_json(dir: &Path, strict: bool) -> std::process::Output {
    let mut cmd = Command::new(cargo_bin());
    cmd.args(["diff", "--dir"])
        .arg(dir)
        .args(["--output", "json", "--offline"]);
    if strict {
        cmd.arg("--strict");
    }
    cmd.output().expect("failed to execute composit diff")
}

#[test]
fn scan_sample_project_produces_valid_report() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("sample-project", tmp.path());

    let out = run_scan(tmp.path());
    assert!(
        out.status.success(),
        "composit scan failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let report_path = tmp.path().join("composit-report.json");
    assert!(report_path.exists(), "report file not written");

    let content = fs::read_to_string(&report_path).unwrap();
    let report: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("report is not valid JSON: {e}"));

    for field in &[
        "workspace",
        "generated",
        "scanner_version",
        "resources",
        "summary",
    ] {
        assert!(
            report.get(field).is_some(),
            "report missing required field: {field}"
        );
    }

    let resources = report["resources"].as_array().expect("resources is array");
    assert!(
        !resources.is_empty(),
        "sample-project should produce at least one resource"
    );

    let total = report
        .pointer("/summary/total_resources")
        .and_then(|v| v.as_u64())
        .expect("summary.total_resources is a number");
    assert_eq!(
        total as usize,
        resources.len(),
        "summary.total_resources must match resources array length"
    );
}

#[test]
fn scan_docker_fixture_finds_compose_services() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("docker", tmp.path());

    let out = run_scan(tmp.path());
    assert!(
        out.status.success(),
        "composit scan failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let compose_paths: Vec<&str> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("docker_compose"))
        .filter_map(|r| r["path"].as_str())
        .collect();
    assert_eq!(
        compose_paths.len(),
        3,
        "expected three docker_compose files (base + override + gpu), got: {compose_paths:?}"
    );
    for needle in &[
        "docker-compose.yml",
        "docker-compose.override.yml",
        "compose.gpu.yml",
    ] {
        assert!(
            compose_paths.iter().any(|p| p.ends_with(needle)),
            "expected compose variant {needle} in paths {compose_paths:?}"
        );
    }

    let services: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("docker_service"))
        .collect();
    // Base (api, worker, db) + override (debug-sidecar; api override merges
    // under the same service name and yields a second entry) + gpu (gpu-worker).
    assert!(
        services.len() >= 5,
        "expected ≥5 docker_service entries across variants, got {}",
        services.len()
    );

    let names: Vec<&str> = services.iter().filter_map(|r| r["name"].as_str()).collect();
    for expected in &["api", "worker", "db", "debug-sidecar", "gpu-worker"] {
        assert!(names.contains(expected), "missing service: {expected}");
    }
}

#[test]
fn scan_env_files_fixture_counts_vars() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("env_files", tmp.path());

    let out = run_scan(tmp.path());
    assert!(
        out.status.success(),
        "composit scan failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let env_resources: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("env_file"))
        .collect();
    assert_eq!(
        env_resources.len(),
        2,
        "expected 2 env_file resources (.env and .env.staging)"
    );

    for r in &env_resources {
        let count = r["variables"]
            .as_u64()
            .expect("env_file resource must have variables count");
        assert!(count > 0, "variables must be > 0");
    }
}

#[test]
fn scan_mcp_config_fixture_finds_cursor_servers() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("mcp_config", tmp.path());

    let out = run_scan(tmp.path());
    assert!(
        out.status.success(),
        "composit scan failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let servers: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("mcp_server"))
        .collect();
    assert_eq!(servers.len(), 3, "expected 3 mcp_server resources");

    let names: Vec<&str> = servers.iter().filter_map(|r| r["name"].as_str()).collect();
    for expected in &["filesystem", "github", "remote-tools"] {
        assert!(names.contains(expected), "missing mcp server: {expected}");
    }

    // remote-tools has a URL → should surface as a provider too
    let providers = report["providers"].as_array().unwrap();
    assert!(
        !providers.is_empty(),
        "remote-tools URL server should appear as a provider"
    );
}

/// Runs `composit scan` followed by `composit diff` on the demo-drift fixture.
/// The fixture is shaped so that exactly three governance rules fire, giving
/// us a stable baseline for the public Show-HN demo artefact.
#[test]
fn scan_jinja_demo_renders_templates_per_inventory() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("jinja-demo", tmp.path());

    let out = run_scan(tmp.path());
    assert!(
        out.status.success(),
        "scan failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();

    let templates: Vec<_> = report["resources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["type"].as_str() == Some("ansible_template"))
        .collect();
    // 3 templates: nginx + app + vault-encrypted.
    assert_eq!(templates.len(), 3);

    // nginx.conf.j2 rendered per inventory: 2 renderings
    let nginx = templates
        .iter()
        .find(|r| r["name"].as_str() == Some("nginx.conf.j2"))
        .expect("nginx template present");
    let nginx_renderings = nginx["renderings"].as_array().unwrap();
    assert_eq!(
        nginx_renderings.len(),
        2,
        "expected one rendering per inventory, got {}",
        nginx_renderings.len()
    );
    let sources: Vec<&str> = nginx_renderings
        .iter()
        .filter_map(|r| r["source"].as_str())
        .collect();
    assert!(sources.iter().any(|s| s.contains("production")));
    assert!(sources.iter().any(|s| s.contains("staging")));

    // Production rendering uses nginx_port=443 from that inventory
    let prod = nginx_renderings
        .iter()
        .find(|r| {
            r["source"]
                .as_str()
                .is_some_and(|s| s.contains("production"))
        })
        .unwrap();
    assert!(prod["rendered"].as_str().unwrap().contains("listen 443;"));

    // Staging uses 8443
    let staging = nginx_renderings
        .iter()
        .find(|r| r["source"].as_str().is_some_and(|s| s.contains("staging")))
        .unwrap();
    assert!(staging["rendered"]
        .as_str()
        .unwrap()
        .contains("listen 8443;"));

    // app.env.j2: dotenv parser kicks in → rendered_parsed.keys populated
    let app = templates
        .iter()
        .find(|r| r["name"].as_str() == Some("app.env.j2"))
        .expect("app template present");
    let app_renderings = app["renderings"].as_array().unwrap();
    assert_eq!(app_renderings.len(), 2);
    let parsed = app_renderings[0]
        .get("rendered_parsed")
        .expect("dotenv parser must produce rendered_parsed");
    assert_eq!(parsed["format"].as_str(), Some("dotenv"));
    let keys = parsed["keys"].as_object().unwrap();
    assert!(keys.contains_key("APP_NAME"));
    assert!(keys.contains_key("APP_DOMAIN"));

    // Vault-encrypted template is surfaced but never rendered
    let vault = templates
        .iter()
        .find(|r| r["name"].as_str() == Some("encrypted.j2"))
        .expect("vault template present");
    assert_eq!(vault["vault_encrypted"].as_bool(), Some(true));
    assert!(vault["renderings"].as_array().unwrap().is_empty());

    // Diff surfaces vault_unsupported + rendered_must_contain PASS.
    // run_scan wrote JSON; rerun with YAML so diff has a report to load.
    let yaml_scan = run_scan_yaml(tmp.path());
    assert!(yaml_scan.status.success());
    let diff = run_diff_json(tmp.path(), false);
    assert!(
        diff.status.success(),
        "diff failed:\n{}",
        String::from_utf8_lossy(&diff.stderr)
    );
    let diff_report: serde_json::Value =
        serde_json::from_str(&String::from_utf8(diff.stdout).unwrap()).unwrap();
    let rules: Vec<&str> = diff_report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["violations"].as_array().unwrap().iter())
        .filter_map(|v| v["rule"].as_str())
        .collect();
    assert!(
        rules.contains(&"vault_unsupported"),
        "vault_unsupported must fire for encrypted.j2: {rules:?}"
    );
    // No template_value_mismatch — both APP_ENV and APP_DOMAIN satisfy
    // their constraints in both inventories.
    assert!(
        !rules.contains(&"template_value_mismatch"),
        "all rendered_must_contain constraints satisfied; got {rules:?}"
    );
}

#[test]
fn scan_resolution_demo_substitutes_env_vars_into_services() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("resolution-demo", tmp.path());

    let out = run_scan(tmp.path());
    assert!(
        out.status.success(),
        "scan failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let services: Vec<_> = report["resources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["type"].as_str() == Some("docker_service"))
        .collect();

    let api = services
        .iter()
        .find(|r| r["name"].as_str() == Some("api"))
        .expect("api service present");
    assert_eq!(
        api["resolved_image"].as_str(),
        Some("ghcr.io/acme/api:1.2.3"),
        "API_TAG from .env must feed into resolved_image"
    );
    // Raw image is preserved alongside the resolved form.
    assert_eq!(
        api["image"].as_str(),
        Some("ghcr.io/acme/api:${API_TAG:-latest}")
    );
    let resolved_ports: Vec<&str> = api["resolved_ports"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(resolved_ports, vec!["8080:8080"]);

    let db = services
        .iter()
        .find(|r| r["name"].as_str() == Some("db"))
        .expect("db service present");
    assert_eq!(db["resolved_image"].as_str(), Some("postgres:16"));

    // ports with no ${VAR} don't get a resolved_ports field — the raw
    // form is already literal.
    assert!(
        db.get("resolved_ports").is_none() || db["resolved_ports"] == serde_json::Value::Null,
        "unchanged ports must not emit resolved_ports"
    );

    // Redaction: secret-looking keys must not leak resolved values. The
    // API_SECRET in .env is never referenced here, but ensure the env
    // reader would have redacted it if it had been.
    let report_str = serde_json::to_string(&report).unwrap();
    assert!(
        !report_str.contains("never-in-the-report"),
        "secret-looking .env values must not appear in report"
    );

    // scan.redact also filters env_file.keys — `METRICS_INTERNAL_URL`
    // matches `*_INTERNAL_URL` from the fixture's redact list and must
    // be replaced with <redacted> in the key list.
    let env_file = report["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["type"].as_str() == Some("env_file"))
        .expect("env_file resource present");
    let keys: Vec<&str> = env_file["keys"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        keys.contains(&"<redacted>"),
        "user-declared redact glob must hide key names in env_file.keys: {keys:?}"
    );
    assert!(
        !keys.contains(&"METRICS_INTERNAL_URL"),
        "raw key name must not leak once redacted: {keys:?}"
    );
    // Non-redacted keys must still be visible.
    assert!(
        keys.contains(&"API_TAG") && keys.contains(&"PG_TAG"),
        "non-matching keys must remain: {keys:?}"
    );

    // RFC 006 report block: env_files_used is populated, and the mystery
    // service's ${MYSTERY_TAG} shows up in unresolved.
    let resolution = report
        .get("resolution")
        .expect("report must carry resolution block when scan opted in");
    let env_files: Vec<&str> = resolution["env_files_used"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        env_files.iter().any(|p| p.ends_with(".env")),
        "env_files_used must list the resolvable .env: {env_files:?}"
    );

    let unresolved = resolution["unresolved"].as_array().unwrap();
    assert!(
        unresolved
            .iter()
            .any(|v| v["variable"].as_str() == Some("MYSTERY_TAG")),
        "MYSTERY_TAG must appear in unresolved list: {unresolved:?}"
    );

    // Diff surfaces unresolved_variable as Info. run_scan wrote JSON;
    // scan for YAML so the diff command finds what it expects to read.
    let yaml_scan = run_scan_yaml(tmp.path());
    assert!(yaml_scan.status.success());
    let diff = run_diff_json(tmp.path(), false);
    assert!(
        diff.status.success(),
        "diff failed:\n{}",
        String::from_utf8_lossy(&diff.stderr)
    );
    let diff_stdout = String::from_utf8(diff.stdout).unwrap();
    let diff_report: serde_json::Value = serde_json::from_str(&diff_stdout).unwrap();
    let unresolved_rules: Vec<&str> = diff_report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["violations"].as_array().unwrap().iter())
        .filter_map(|v| v["rule"].as_str())
        .filter(|r| *r == "unresolved_variable")
        .collect();
    assert!(
        !unresolved_rules.is_empty(),
        "composit diff must surface unresolved_variable for MYSTERY_TAG"
    );
}

#[test]
fn scan_ansible_fixture_finds_playbook_inventory_and_role() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("ansible", tmp.path());

    let out = run_scan(tmp.path());
    assert!(
        out.status.success(),
        "composit scan failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    // Playbook with 2 plays + both tasks + roles ref
    let playbooks: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("ansible_playbook"))
        .collect();
    assert_eq!(playbooks.len(), 1, "expected one ansible_playbook");
    // Resource.extra is flattened into the top-level JSON object.
    let plays_count = playbooks[0]["plays"].as_u64().unwrap();
    assert_eq!(plays_count, 2, "site.yml has two plays");

    // Inventory with group names
    let inventories: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("ansible_inventory"))
        .collect();
    assert_eq!(inventories.len(), 1);
    let group_names: Vec<&str> = inventories[0]["group_names"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(group_names.contains(&"all"));

    // Role with handlers + templates
    let roles: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("ansible_role"))
        .collect();
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0]["name"].as_str(), Some("nginx"));
    assert_eq!(roles[0]["has_handlers"].as_bool(), Some(true));
    assert_eq!(roles[0]["has_templates"].as_bool(), Some(true));
    assert_eq!(roles[0]["template_count"].as_u64(), Some(1));

    // Critical: the role's tasks/main.yml must NOT double-register as a
    // playbook (it's a list of tasks, not plays — but we also dedupe via
    // claimed paths). No duplicate ansible_playbook matching roles path.
    let playbook_paths: Vec<&str> = playbooks
        .iter()
        .filter_map(|r| r["path"].as_str())
        .collect();
    for p in &playbook_paths {
        assert!(
            !p.contains("roles/"),
            "role task file leaked into playbook list: {p}"
        );
    }
}

#[test]
fn roles_demo_surfaces_rfc005_violations() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("roles-demo", tmp.path());

    let scan = run_scan_yaml(tmp.path());
    assert!(
        scan.status.success(),
        "scan failed:\n{}",
        String::from_utf8_lossy(&scan.stderr)
    );

    let diff = run_diff_json(tmp.path(), false);
    assert!(
        diff.status.success(),
        "diff failed:\n{}",
        String::from_utf8_lossy(&diff.stderr)
    );

    let stdout = String::from_utf8(diff.stdout).unwrap();
    let report: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("diff JSON parse failed: {e}\nstdout:\n{stdout}"));

    let mut rules: Vec<String> = Vec::new();
    for cat in report["categories"].as_array().unwrap() {
        for v in cat["violations"].as_array().unwrap() {
            rules.push(v["rule"].as_str().unwrap().to_string());
        }
    }

    // Each role_* rule must fire at least once. This locks the RFC 005
    // diff contract — if the renderer or checker drops one of these we
    // hear about it.
    for expected in &[
        "role_image_not_pinned",      // postgres-db uses :latest
        "role_port_missing",          // postgres-db doesn't expose 5432
        "role_network_missing",       // postgres-db not on backend network
        "role_image_prefix_mismatch", // api image uses ghcr.io/other/
        "role_env_var_missing",       // .env.production lacks DATABASE_URL
        "role_env_var_forbidden",     // .env.production sets DEBUG
        "role_count_below_min",       // 0 frontend-* services found, min=2
    ] {
        assert!(
            rules.iter().any(|r| r == expected),
            "roles-demo must raise {expected}, got: {rules:?}"
        );
    }

    // role_count_below_min must populate details with the matched summary
    // (even if it's "(no matches)") so HTML shows what was considered.
    let count_min: Vec<&serde_json::Value> = report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["violations"].as_array().unwrap().iter())
        .filter(|v| v["rule"].as_str() == Some("role_count_below_min"))
        .collect();
    assert_eq!(count_min.len(), 1);
    let details = count_min[0]["details"].as_str().unwrap();
    assert!(
        details.contains("matched:") && details.contains("(no matches)"),
        "count_below_min details must list matched resources: {details}"
    );

    // Expected/actual fields must be populated on every role_* violation so
    // the HTML diff renderer has SOLL/IST data to display.
    for cat in report["categories"].as_array().unwrap() {
        for v in cat["violations"].as_array().unwrap() {
            let rule = v["rule"].as_str().unwrap();
            if rule.starts_with("role_") {
                assert!(
                    v.get("expected").is_some() && v.get("actual").is_some(),
                    "{rule} must carry expected+actual, got: {v}"
                );
            }
        }
    }
}

#[test]
fn demo_drift_surfaces_three_expected_errors() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("demo-drift", tmp.path());

    let scan = run_scan_yaml(tmp.path());
    assert!(
        scan.status.success(),
        "scan failed:\n{}",
        String::from_utf8_lossy(&scan.stderr)
    );

    // Non-strict diff: exit 0 so CI stays green while we inspect the payload.
    let diff = run_diff_json(tmp.path(), false);
    assert!(
        diff.status.success(),
        "diff failed:\n{}",
        String::from_utf8_lossy(&diff.stderr)
    );

    let stdout = String::from_utf8(diff.stdout).unwrap();
    let report: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("diff JSON parse failed: {e}\nstdout:\n{stdout}"));

    let summary = &report["summary"];
    assert_eq!(summary["errors"].as_u64(), Some(3), "expected 3 errors");
    assert_eq!(summary["warnings"].as_u64(), Some(0));
    assert_eq!(summary["info"].as_u64(), Some(0));

    // Collect all violation rule names across categories.
    let mut rules: Vec<String> = Vec::new();
    for cat in report["categories"].as_array().unwrap() {
        for v in cat["violations"].as_array().unwrap() {
            rules.push(v["rule"].as_str().unwrap().to_string());
        }
    }

    for expected in &[
        "unapproved_provider",       // rogue-tools MCP server
        "image_not_allowed",         // redis:latest
        "required_resource_missing", // no workflow
    ] {
        assert!(
            rules.iter().any(|r| r == expected),
            "demo-drift must raise {expected}, got: {rules:?}"
        );
    }
}

#[test]
fn scan_kubernetes_fixture_finds_manifests_kustomize_and_helm() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("kubernetes", tmp.path());

    let out = run_scan(tmp.path());
    assert!(
        out.status.success(),
        "composit scan failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let manifests: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("kubernetes_manifest"))
        .collect();
    assert_eq!(
        manifests.len(),
        2,
        "expected Deployment + Service from deployment.yaml"
    );
    let names: Vec<&str> = manifests
        .iter()
        .filter_map(|r| r["name"].as_str())
        .collect();
    assert!(names.contains(&"Deployment/api"));
    assert!(names.contains(&"Service/api"));

    let kustomizations: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("kustomization"))
        .collect();
    assert_eq!(kustomizations.len(), 1);
    assert_eq!(
        kustomizations[0]["namespace"].as_str(),
        Some("widgetshop"),
        "kustomization carries namespace"
    );

    let charts: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("helm_chart"))
        .collect();
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0]["name"].as_str(), Some("widgetshop"));
    assert_eq!(charts[0]["chart_version"].as_str(), Some("0.3.1"));
}

#[test]
fn scan_nginx_fixture_finds_config_and_upstreams() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("nginx", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let nginx: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("nginx_config"))
        .collect();
    assert_eq!(nginx.len(), 1, "expected one nginx_config resource");
    assert_eq!(
        nginx[0]["server_blocks"].as_u64(),
        Some(2),
        "two server blocks in fixture"
    );
    assert_eq!(nginx[0]["upstreams"].as_u64(), Some(1));
    assert_eq!(nginx[0]["proxy_pass"].as_bool(), Some(true));
    assert_eq!(nginx[0]["ssl"].as_bool(), Some(true));
}

#[test]
fn scan_opa_policy_fixture_finds_package_and_entrypoints() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("opa_policy", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let policies: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("opa_policy"))
        .collect();
    assert_eq!(policies.len(), 1);
    assert_eq!(policies[0]["name"].as_str(), Some("widgetshop.access"));
    let entrypoints = policies[0]["entrypoints"].as_array().unwrap();
    let names: Vec<&str> = entrypoints.iter().filter_map(|v| v.as_str()).collect();
    assert!(names.contains(&"allow"));
    assert!(names.contains(&"deny"));
}

#[test]
fn scan_grafana_fixture_finds_dashboard_datasources_and_providers() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("grafana", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let dashboards: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("grafana_dashboard"))
        .collect();
    assert_eq!(dashboards.len(), 1);
    assert_eq!(dashboards[0]["name"].as_str(), Some("API Latency"));
    assert_eq!(dashboards[0]["panels"].as_u64(), Some(3));

    let datasources: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("grafana_datasource"))
        .collect();
    assert_eq!(datasources.len(), 2);
    let ds_names: Vec<&str> = datasources
        .iter()
        .filter_map(|r| r["name"].as_str())
        .collect();
    assert!(ds_names.contains(&"Prometheus"));
    assert!(ds_names.contains(&"Loki"));

    let providers: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("grafana_dashboard_provider"))
        .collect();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["name"].as_str(), Some("default"));
}

#[test]
fn scan_fly_toml_fixture_finds_app_and_region() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("fly_toml", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let apps: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("fly_app"))
        .collect();
    assert_eq!(apps.len(), 1, "expected one fly_app resource");
    assert_eq!(apps[0]["name"].as_str(), Some("my-api"));
    assert_eq!(apps[0]["primary_region"].as_str(), Some("fra"));
    assert_eq!(apps[0]["http_service"].as_bool(), Some(true));
}

#[test]
fn scan_render_yaml_fixture_finds_services() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("render_yaml", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let services: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("render_service"))
        .collect();
    assert_eq!(services.len(), 3, "expected 3 render_service resources");

    let names: Vec<&str> = services.iter().filter_map(|r| r["name"].as_str()).collect();
    assert!(names.contains(&"api"));
    assert!(names.contains(&"worker"));
    assert!(names.contains(&"scheduler"));

    let types: Vec<&str> = services
        .iter()
        .filter_map(|r| r["service_type"].as_str())
        .collect();
    assert!(types.contains(&"web"));
    assert!(types.contains(&"worker"));
    assert!(types.contains(&"cron"));
}

#[test]
fn scan_vercel_json_fixture_finds_config() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("vercel_json", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let configs: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("vercel_config"))
        .collect();
    assert_eq!(configs.len(), 1, "expected one vercel_config resource");
    assert_eq!(configs[0]["framework"].as_str(), Some("nextjs"));
    assert_eq!(configs[0]["rewrites"].as_u64(), Some(1));
    assert_eq!(configs[0]["redirects"].as_u64(), Some(2));
    assert_eq!(configs[0]["headers"].as_u64(), Some(1));
    assert_eq!(configs[0]["functions"].as_u64(), Some(2));
}

#[test]
fn scan_skaffold_fixture_finds_config() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("skaffold", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let configs: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("skaffold_config"))
        .collect();
    assert_eq!(configs.len(), 1, "expected one skaffold_config resource");
    assert_eq!(configs[0]["name"].as_str(), Some("widgetshop"));
    assert_eq!(configs[0]["artifacts"].as_u64(), Some(2));
    assert_eq!(configs[0]["profiles"].as_u64(), Some(2));
    assert_eq!(configs[0]["deploy_type"].as_str(), Some("helm"));
}

#[test]
fn scan_traefik_fixture_finds_config() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("traefik", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let configs: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("traefik_config"))
        .collect();
    assert_eq!(configs.len(), 1, "expected one traefik_config resource");

    let entrypoints = configs[0]["entrypoints"].as_array().unwrap();
    let ep_names: Vec<&str> = entrypoints.iter().filter_map(|v| v.as_str()).collect();
    assert!(ep_names.contains(&"web"));
    assert!(ep_names.contains(&"websecure"));
    assert_eq!(configs[0]["dashboard"].as_bool(), Some(true));
    assert_eq!(configs[0]["tls"].as_bool(), Some(true));
}

#[test]
fn scan_proto_fixture_finds_file() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("proto", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let protos: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("proto_file"))
        .collect();
    assert_eq!(protos.len(), 1, "expected one proto_file resource");
    assert_eq!(protos[0]["name"].as_str(), Some("widgetshop.v1"));
    assert_eq!(protos[0]["syntax"].as_str(), Some("proto3"));
    assert_eq!(protos[0]["services"].as_u64(), Some(1));
    assert_eq!(protos[0]["messages"].as_u64(), Some(4));
    assert_eq!(protos[0]["rpcs"].as_u64(), Some(3));
}

#[test]
fn scan_tempo_fixture_finds_config() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("tempo", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let configs: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("tempo_config"))
        .collect();
    assert_eq!(configs.len(), 1, "expected one tempo_config resource");
    assert_eq!(configs[0]["storage_backend"].as_str(), Some("s3"));

    let receivers = configs[0]["receivers"].as_array().unwrap();
    let names: Vec<&str> = receivers.iter().filter_map(|v| v.as_str()).collect();
    assert!(names.contains(&"otlp"));
    assert!(names.contains(&"jaeger"));
}

#[test]
fn scan_db_migrations_fixture_finds_alembic_and_prisma() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("db_migrations", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let migrations: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("db_migrations"))
        .collect();
    assert!(
        migrations.len() >= 2,
        "expected alembic and prisma resources"
    );

    let frameworks: Vec<&str> = migrations
        .iter()
        .filter_map(|r| r["name"].as_str())
        .collect();
    assert!(frameworks.contains(&"alembic"), "missing alembic");
    assert!(frameworks.contains(&"prisma"), "missing prisma");

    let alembic = migrations
        .iter()
        .find(|r| r["name"].as_str() == Some("alembic"))
        .unwrap();
    assert_eq!(alembic["migration_count"].as_u64(), Some(2));
}

#[test]
fn scan_deploy_scripts_fixture_finds_scripts() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("deploy_scripts", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let scripts: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("deploy_script"))
        .collect();
    assert!(scripts.len() >= 2, "expected deploy.sh and bootstrap.sh");

    let names: Vec<&str> = scripts.iter().filter_map(|r| r["name"].as_str()).collect();
    assert!(names.contains(&"deploy.sh"));
    assert!(names.contains(&"bootstrap.sh"));

    let bootstrap = scripts
        .iter()
        .find(|r| r["name"].as_str() == Some("bootstrap.sh"))
        .unwrap();
    assert_eq!(bootstrap["kind"].as_str(), Some("bootstrap"));
}

#[test]
fn opa_policy_eval_fires_deny_for_latest_tag() {
    // Full scan → diff pipeline: a deny rule that checks for :latest tags
    // must produce a policy_violation Error for the offending service.
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("opa-policy-eval", tmp.path());

    let scan = run_scan_yaml(tmp.path());
    assert!(
        scan.status.success(),
        "scan failed:\n{}",
        String::from_utf8_lossy(&scan.stderr)
    );

    let diff = run_diff_json(tmp.path(), false);
    assert!(
        diff.status.success(),
        "diff failed:\n{}",
        String::from_utf8_lossy(&diff.stderr)
    );

    let stdout = String::from_utf8(diff.stdout).unwrap();
    let report: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("diff JSON parse failed: {e}\nstdout:\n{stdout}"));

    // Collect all violations across categories.
    let mut violations: Vec<serde_json::Value> = Vec::new();
    for cat in report["categories"].as_array().unwrap() {
        for v in cat["violations"].as_array().unwrap() {
            violations.push(v.clone());
        }
    }

    let policy_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("policy_violation"))
        .collect();

    assert_eq!(
        policy_violations.len(),
        1,
        "expected exactly one policy_violation (worker uses :latest); got: {:#?}",
        policy_violations
    );

    let msg = policy_violations[0]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("worker"),
        "violation message should name the offending service; got: {msg}"
    );
    assert!(
        msg.contains(":latest"),
        "violation message should mention :latest; got: {msg}"
    );

    // A policy that produced violations must NOT also emit policy_passed.
    let passed: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("policy_passed"))
        .collect();
    assert!(
        passed.is_empty(),
        "a policy that fired violations must not also emit policy_passed"
    );
}

#[test]
fn opa_policy_eval_clean_when_all_images_pinned() {
    use std::fs;

    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("opa-policy-eval", tmp.path());

    // Overwrite docker-compose.yml with all services pinned.
    fs::write(
        tmp.path().join("docker-compose.yml"),
        "services:\n  api:\n    image: ghcr.io/widgetshop/api:1.4.2\n  db:\n    image: postgres:16\n",
    )
    .unwrap();

    assert!(run_scan_yaml(tmp.path()).status.success());

    let diff = run_diff_json(tmp.path(), false);
    let stdout = String::from_utf8(diff.stdout).unwrap();
    let report: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("diff JSON parse failed: {e}\nstdout:\n{stdout}"));

    let violations: Vec<_> = report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["violations"].as_array().unwrap().iter())
        .filter(|v| v["rule"].as_str() == Some("policy_violation"))
        .collect();

    assert!(
        violations.is_empty(),
        "expected no policy_violation when all images are pinned; got: {:#?}",
        violations
    );
}

#[test]
fn demo_drift_exits_nonzero_in_strict_mode() {
    // `--strict` is the CI gate: errors must fail the pipeline. Guards
    // against a regression where the diff report lists errors but the
    // process still exits 0.
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("demo-drift", tmp.path());

    assert!(run_scan_yaml(tmp.path()).status.success());

    let diff = run_diff_json(tmp.path(), true);
    assert_eq!(
        diff.status.code(),
        Some(1),
        "strict diff with errors must exit 1"
    );
}

// ─────────────────────────────────────────────────────────
// Scanners shipped after v0.3.2 — agent_spec, cargo_manifest, go_module
// ─────────────────────────────────────────────────────────

#[test]
fn scan_agent_spec_fixture_finds_skill_and_agents() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("agent_spec", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let specs: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("agent_spec"))
        .collect();

    // SKILL.md (frontmatter), AGENTS.md, skills/heading-only/SKILL.md.
    // docs/SKILL.md must NOT appear (no frontmatter, no skill heading).
    assert_eq!(
        specs.len(),
        3,
        "expected exactly 3 agent_spec resources; got: {:#?}",
        specs.iter().map(|r| r["path"].as_str()).collect::<Vec<_>>()
    );

    // Top-level SKILL.md with frontmatter.
    let widget = specs
        .iter()
        .find(|r| r["name"].as_str() == Some("widget-skill"))
        .expect("widget-skill from frontmatter name");
    assert_eq!(widget["kind"].as_str(), Some("skill"));
    assert_eq!(widget["version"].as_str(), Some("1.2.3"));
    let desc = widget["description"].as_str().unwrap();
    assert!(
        desc.contains("multi-line") && desc.contains("collapse"),
        "folded scalar must collapse to one line; got: {desc}"
    );
    assert!(widget["allowed_tools"]
        .as_str()
        .is_some_and(|s| s.contains("WebSearch")));

    // AGENTS.md (free-form, no frontmatter).
    let agents = specs
        .iter()
        .find(|r| r["kind"].as_str() == Some("agents"))
        .expect("AGENTS.md detected");
    assert_eq!(agents["path"].as_str(), Some("./AGENTS.md"));

    // Heading-only SKILL.md fallback.
    let heading = specs
        .iter()
        .find(|r| {
            r["path"]
                .as_str()
                .is_some_and(|p| p.contains("heading-only"))
        })
        .expect("heading-only SKILL.md falls back to dir name");
    assert_eq!(heading["name"].as_str(), Some("heading-only"));

    // Bare SKILL.md without frontmatter or heading must be excluded.
    assert!(
        !specs.iter().any(|r| r["path"]
            .as_str()
            .is_some_and(|p| p.ends_with("docs/SKILL.md"))),
        "docs/SKILL.md without frontmatter or skill heading must NOT be recorded"
    );
}

#[test]
fn scan_cargo_manifest_fixture_finds_workspace_and_crates() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("cargo_manifest", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    // Workspace root: 1 cargo_workspace (root Cargo.toml has no [package]).
    let workspaces: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("cargo_workspace"))
        .collect();
    assert_eq!(workspaces.len(), 1);
    let ws = workspaces[0];
    assert_eq!(ws["member_count"].as_u64(), Some(2));
    assert_eq!(ws["version"].as_str(), Some("0.1.0"));

    // Member crates: 2.
    let crates: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("cargo_crate"))
        .collect();
    assert_eq!(crates.len(), 2);

    let foo = crates
        .iter()
        .find(|r| r["name"].as_str() == Some("foo"))
        .expect("foo crate");
    assert_eq!(foo["version"].as_str(), Some("0.2.0"));
    assert_eq!(foo["edition"].as_str(), Some("2021"));
    assert_eq!(foo["license"].as_str(), Some("MIT"));

    // bar has no license — that field must be absent, not "null".
    let bar = crates
        .iter()
        .find(|r| r["name"].as_str() == Some("bar"))
        .expect("bar crate");
    assert!(
        bar.get("license").is_none() || bar["license"].is_null(),
        "absent license should not appear; got: {:?}",
        bar.get("license")
    );
}

#[test]
fn scan_go_module_fixture_finds_module_and_requires() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("go_module", tmp.path());

    let out = run_scan(tmp.path());
    assert!(out.status.success());

    let content = fs::read_to_string(tmp.path().join("composit-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();
    let resources = report["resources"].as_array().unwrap();

    let modules: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("go_module"))
        .collect();
    // Multi-module repo: root go.mod + subapp/go.mod.
    assert_eq!(modules.len(), 2);

    let root = modules
        .iter()
        .find(|r| r["name"].as_str() == Some("example.com/widgetshop"))
        .expect("root module");
    assert_eq!(root["go_version"].as_str(), Some("1.22"));
    assert_eq!(root["direct_requires"].as_u64(), Some(2));
    assert_eq!(root["indirect_requires"].as_u64(), Some(2));

    let sub = modules
        .iter()
        .find(|r| r["name"].as_str() == Some("example.com/widgetshop/subapp"))
        .expect("subapp module");
    assert_eq!(sub["direct_requires"].as_u64(), Some(1));
}
