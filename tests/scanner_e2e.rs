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
    let report: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("report is not valid JSON: {e}"));

    for field in &["workspace", "generated", "scanner_version", "resources", "summary"] {
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
    assert_eq!(services.len(), 3, "expected 3 docker_service resources (api, worker, db)");

    let names: Vec<&str> = services
        .iter()
        .filter_map(|r| r["name"].as_str())
        .collect();
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
    assert_eq!(env_resources.len(), 2, "expected 2 env_file resources (.env and .env.staging)");

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
        "unapproved_provider",      // rogue-tools MCP server
        "image_not_allowed",        // redis:latest
        "required_resource_missing", // no workflow
    ] {
        assert!(
            rules.iter().any(|r| r == expected),
            "demo-drift must raise {expected}, got: {rules:?}"
        );
    }
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
