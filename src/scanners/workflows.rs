use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

/// Supported CI/CD workflow platforms and their file patterns.
const WORKFLOW_PATTERNS: &[(&str, &str)] = &[
    (".github/workflows/*.yml", "github-actions"),
    (".github/workflows/*.yaml", "github-actions"),
    (".forgejo/workflows/*.yml", "forgejo"),
    (".forgejo/workflows/*.yaml", "forgejo"),
    (".gitea/workflows/*.yml", "gitea"),
    (".gitea/workflows/*.yaml", "gitea"),
];

pub struct WorkflowScanner;

#[async_trait]
impl Scanner for WorkflowScanner {
    fn id(&self) -> &str {
        "workflows"
    }

    fn name(&self) -> &str {
        "CI/CD Workflow Scanner"
    }

    fn description(&self) -> &str {
        "Detects CI/CD workflows (GitHub Actions, Forgejo, Gitea)"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        for (pattern, platform) in WORKFLOW_PATTERNS {
            let full_pattern = context.dir.join(pattern);
            for entry in glob(&full_pattern.to_string_lossy())? {
                if let Ok(path) = entry {
                    let rel_path = path
                        .strip_prefix(&context.dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();
                    let display_path = format!("./{}", rel_path);

                    let content = match std::fs::read_to_string(&path) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!(
                                "warning: workflows scanner could not read {}: {}",
                                path.display(),
                                e
                            );
                            continue;
                        }
                    };

                    let yaml: serde_yaml::Value = match serde_yaml::from_str(&content) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!(
                                "warning: workflows scanner could not parse {}: {}",
                                path.display(),
                                e
                            );
                            continue;
                        }
                    };

                    let workflow_name = yaml
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("unnamed")
                        .to_string();

                    let mut extra = HashMap::new();
                    extra.insert(
                        "platform".to_string(),
                        serde_json::Value::String(platform.to_string()),
                    );

                    // Extract triggers
                    let triggers = extract_triggers(&yaml);
                    if !triggers.is_empty() {
                        extra.insert(
                            "triggers".to_string(),
                            serde_json::Value::Array(
                                triggers
                                    .into_iter()
                                    .map(serde_json::Value::String)
                                    .collect(),
                            ),
                        );
                    }

                    // Extract job names
                    let jobs = extract_jobs(&yaml);
                    extra.insert(
                        "jobs".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(jobs.len())),
                    );
                    if !jobs.is_empty() {
                        extra.insert(
                            "job_names".to_string(),
                            serde_json::Value::Array(
                                jobs.into_iter()
                                    .map(serde_json::Value::String)
                                    .collect(),
                            ),
                        );
                    }

                    // Extract runner
                    if let Some(runner) = extract_first_runner(&yaml) {
                        extra.insert(
                            "runs_on".to_string(),
                            serde_json::Value::String(runner),
                        );
                    }

                    resources.push(Resource {
                        resource_type: "workflow".to_string(),
                        name: Some(workflow_name),
                        path: Some(display_path),
                        provider: None,
                        created: None,
                        created_by: None,
                        detected_by: "workflows".to_string(),
                        estimated_cost: None,
                        extra,
                    });
                }
            }
        }

        // Also detect .gitlab-ci.yml (single file, different format)
        let gitlab_ci = context.dir.join(".gitlab-ci.yml");
        if gitlab_ci.exists() {
            if let Ok(content) = std::fs::read_to_string(&gitlab_ci) {
                if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    let stages = yaml
                        .get("stages")
                        .and_then(|s| s.as_sequence())
                        .map(|s| s.len())
                        .unwrap_or(0);

                    let mut extra = HashMap::new();
                    extra.insert(
                        "platform".to_string(),
                        serde_json::Value::String("gitlab-ci".to_string()),
                    );
                    if stages > 0 {
                        extra.insert(
                            "stages".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(stages)),
                        );
                    }

                    // Count jobs (top-level keys that aren't reserved)
                    let job_count = yaml
                        .as_mapping()
                        .map(|m| {
                            m.keys()
                                .filter_map(|k| k.as_str())
                                .filter(|k| !is_gitlab_reserved_key(k))
                                .count()
                        })
                        .unwrap_or(0);
                    extra.insert(
                        "jobs".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(job_count)),
                    );

                    resources.push(Resource {
                        resource_type: "workflow".to_string(),
                        name: Some("GitLab CI".to_string()),
                        path: Some("./.gitlab-ci.yml".to_string()),
                        provider: None,
                        created: None,
                        created_by: None,
                        detected_by: "workflows".to_string(),
                        estimated_cost: None,
                        extra,
                    });
                }
            }
        }

        Ok(ScanResult {
            resources,
            providers: vec![],
        })
    }
}

fn extract_triggers(yaml: &serde_yaml::Value) -> Vec<String> {
    // YAML spec: bare `on` is parsed as boolean `true` by some parsers
    let on = match yaml.get("on").or_else(|| yaml.get(serde_yaml::Value::Bool(true))) {
        Some(v) => v,
        None => return vec![],
    };

    if let Some(s) = on.as_str() {
        return vec![s.to_string()];
    }
    if let Some(seq) = on.as_sequence() {
        return seq
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
    }
    if let Some(map) = on.as_mapping() {
        return map
            .keys()
            .filter_map(|k| k.as_str().map(|s| s.to_string()))
            .collect();
    }

    vec![]
}

fn extract_jobs(yaml: &serde_yaml::Value) -> Vec<String> {
    yaml.get("jobs")
        .and_then(|j| j.as_mapping())
        .map(|m| {
            m.keys()
                .filter_map(|k| k.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_first_runner(yaml: &serde_yaml::Value) -> Option<String> {
    let jobs = yaml.get("jobs")?.as_mapping()?;
    for (_, job) in jobs {
        if let Some(runner) = job.get("runs-on").and_then(|r| r.as_str()) {
            return Some(runner.to_string());
        }
    }
    None
}

fn is_gitlab_reserved_key(key: &str) -> bool {
    matches!(
        key,
        "stages"
            | "variables"
            | "default"
            | "include"
            | "image"
            | "services"
            | "cache"
            | "before_script"
            | "after_script"
            | "workflow"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_actions_workflow() {
        let yaml: serde_yaml::Value = serde_yaml::from_str(
            r#"
name: CI
on:
  push:
    branches: [main]
  pull_request:

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm test
  lint:
    runs-on: ubuntu-latest
    steps:
      - run: npm run lint
"#,
        )
        .unwrap();

        let triggers = extract_triggers(&yaml);
        assert_eq!(triggers.len(), 2);
        assert!(triggers.contains(&"push".to_string()));
        assert!(triggers.contains(&"pull_request".to_string()));

        let jobs = extract_jobs(&yaml);
        assert_eq!(jobs.len(), 2);
        assert!(jobs.contains(&"build".to_string()));
        assert!(jobs.contains(&"lint".to_string()));

        let runner = extract_first_runner(&yaml);
        assert_eq!(runner.as_deref(), Some("ubuntu-latest"));
    }

    #[test]
    fn test_parse_forgejo_workflow() {
        let yaml: serde_yaml::Value = serde_yaml::from_str(
            r#"
name: Deploy
on:
  push:
    branches: [main]
    paths:
      - 'services/**'

jobs:
  deploy:
    runs-on: self-hosted
    steps:
      - name: Deploy
        run: ./deploy.sh
"#,
        )
        .unwrap();

        let triggers = extract_triggers(&yaml);
        assert_eq!(triggers, vec!["push".to_string()]);

        let jobs = extract_jobs(&yaml);
        assert_eq!(jobs, vec!["deploy".to_string()]);

        let runner = extract_first_runner(&yaml);
        assert_eq!(runner.as_deref(), Some("self-hosted"));
    }

    #[test]
    fn test_parse_workflow_dispatch() {
        let yaml: serde_yaml::Value = serde_yaml::from_str(
            r#"
name: Manual Build
on:
  workflow_dispatch:

jobs:
  rebuild:
    runs-on: builder
    steps:
      - run: make build
"#,
        )
        .unwrap();

        let triggers = extract_triggers(&yaml);
        assert_eq!(triggers, vec!["workflow_dispatch".to_string()]);
    }

    #[test]
    fn test_gitlab_reserved_keys() {
        assert!(is_gitlab_reserved_key("stages"));
        assert!(is_gitlab_reserved_key("variables"));
        assert!(!is_gitlab_reserved_key("build"));
        assert!(!is_gitlab_reserved_key("deploy"));
    }
}
