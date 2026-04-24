use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use glob::glob;

use crate::core::scanner::{ScanContext, ScanResult, Scanner};
use crate::core::types::Resource;

pub struct ProtoScanner;

#[async_trait]
impl Scanner for ProtoScanner {
    fn id(&self) -> &str {
        "proto"
    }

    fn name(&self) -> &str {
        "Protobuf/gRPC Scanner"
    }

    fn description(&self) -> &str {
        "Detects .proto files — package, syntax, service and message counts"
    }

    fn needs_network(&self) -> bool {
        false
    }

    async fn scan(&self, context: &ScanContext) -> Result<ScanResult> {
        let mut resources = Vec::new();

        let pattern = context.dir.join("**/*.proto");
        for entry in glob(&pattern.to_string_lossy())?.flatten() {
            if context.is_excluded(&entry) || !entry.is_file() {
                continue;
            }
            if let Some(r) = parse_proto_file(&entry, &context.dir) {
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

fn parse_proto_file(path: &Path, base_dir: &Path) -> Option<Resource> {
    let content = std::fs::read_to_string(path).ok()?;

    let syntax = extract_syntax(&content);
    // Require a syntax declaration to avoid claiming arbitrary .proto-named
    // files that aren't really protobuf definitions.
    syntax.as_deref()?;

    let rel = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let package = extract_value(&content, "package");
    let (services, messages, rpcs) = summarise(&content);

    let mut extra = HashMap::new();
    if let Some(ref s) = syntax {
        extra.insert("syntax".to_string(), serde_json::Value::String(s.clone()));
    }
    extra.insert(
        "services".to_string(),
        serde_json::Value::Number(serde_json::Number::from(services)),
    );
    extra.insert(
        "messages".to_string(),
        serde_json::Value::Number(serde_json::Number::from(messages)),
    );
    extra.insert(
        "rpcs".to_string(),
        serde_json::Value::Number(serde_json::Number::from(rpcs)),
    );

    Some(Resource {
        resource_type: "proto_file".to_string(),
        name: package,
        path: Some(format!("./{}", rel)),
        provider: None,
        created: None,
        created_by: None,
        detected_by: "proto".to_string(),
        estimated_cost: None,
        extra,
    })
}

fn extract_syntax(content: &str) -> Option<String> {
    for raw in content.lines() {
        let line = raw.trim();
        if line.starts_with("//") || line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("syntax") {
            let val = rest
                .trim()
                .trim_start_matches('=')
                .trim()
                .trim_end_matches(';')
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn extract_value(content: &str, keyword: &str) -> Option<String> {
    for raw in content.lines() {
        let line = raw.trim();
        if line.starts_with("//") {
            continue;
        }
        if let Some(rest) = line.strip_prefix(keyword) {
            let val = rest
                .trim()
                .trim_start_matches('=')
                .trim()
                .trim_end_matches(';')
                .trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn summarise(content: &str) -> (usize, usize, usize) {
    let mut services = 0usize;
    let mut messages = 0usize;
    let mut rpcs = 0usize;

    for raw in content.lines() {
        let line = raw.trim();
        if line.starts_with("//") {
            continue;
        }
        if line.starts_with("service ") && line.contains('{') {
            services += 1;
        } else if line.starts_with("message ") && line.contains('{') {
            messages += 1;
        } else if line.trim_start().starts_with("rpc ") {
            rpcs += 1;
        }
    }

    (services, messages, rpcs)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
syntax = "proto3";

package widgetshop.v1;

import "google/protobuf/timestamp.proto";

message Widget {
  string id = 1;
  string name = 2;
}

message ListWidgetsRequest {}
message ListWidgetsResponse {
  repeated Widget widgets = 1;
}

service WidgetService {
  rpc GetWidget(Widget) returns (Widget);
  rpc ListWidgets(ListWidgetsRequest) returns (ListWidgetsResponse);
}
"#;

    #[test]
    fn extracts_syntax_and_package() {
        assert_eq!(extract_syntax(FIXTURE), Some("proto3".to_string()));
        assert_eq!(
            extract_value(FIXTURE, "package"),
            Some("widgetshop.v1".to_string())
        );
    }

    #[test]
    fn counts_services_messages_rpcs() {
        let (services, messages, rpcs) = summarise(FIXTURE);
        assert_eq!(services, 1);
        assert_eq!(messages, 3);
        assert_eq!(rpcs, 2);
    }

    #[test]
    fn no_syntax_returns_none() {
        let content = "message Foo {}\n";
        assert!(extract_syntax(content).is_none());
    }
}
