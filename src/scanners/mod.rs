pub mod caddyfile;
pub mod cron;
pub mod docker;
pub mod env_files;
pub mod extra_patterns;
pub mod kubernetes;
pub mod mcp_config;
pub mod mcp_provider;
pub mod prometheus;
pub mod terraform;
pub mod workflows;

use crate::core::registry::ScannerRegistry;

pub fn register_default_scanners(registry: &mut ScannerRegistry) {
    registry.register(Box::new(docker::DockerScanner));
    registry.register(Box::new(env_files::EnvFilesScanner));
    registry.register(Box::new(terraform::TerraformScanner));
    registry.register(Box::new(caddyfile::CaddyfileScanner));
    registry.register(Box::new(workflows::WorkflowScanner));
    registry.register(Box::new(prometheus::PrometheusScanner));
    registry.register(Box::new(cron::CronScanner));
    registry.register(Box::new(kubernetes::KubernetesScanner));
    registry.register(Box::new(mcp_config::McpConfigScanner));
    registry.register(Box::new(mcp_provider::McpProviderScanner));
}
