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

    let compose = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("docker_compose"))
        .count();
    assert_eq!(compose, 1, "expected one docker_compose resource");

    let services: Vec<_> = resources
        .iter()
        .filter(|r| r["type"].as_str() == Some("docker_service"))
        .collect();
    assert_eq!(
        services.len(),
        3,
        "expected 3 docker_service resources (api, worker, db)"
    );

    let names: Vec<&str> = services.iter().filter_map(|r| r["name"].as_str()).collect();
    for expected in &["api", "worker", "db"] {
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
    assert!(migrations.len() >= 2, "expected alembic and prisma resources");

    let frameworks: Vec<&str> = migrations.iter().filter_map(|r| r["name"].as_str()).collect();
    assert!(frameworks.contains(&"alembic"), "missing alembic");
    assert!(frameworks.contains(&"prisma"), "missing prisma");

    let alembic = migrations.iter().find(|r| r["name"].as_str() == Some("alembic")).unwrap();
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

    let bootstrap = scripts.iter().find(|r| r["name"].as_str() == Some("bootstrap.sh")).unwrap();
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
