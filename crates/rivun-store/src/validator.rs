use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use rivun_policy::PolicySet;
use rivun_router::RouteTable;

use crate::bundle::DomainPackBundle;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackValidationResult {
    pub valid: bool,
    pub syntax_errors: Vec<String>,
    pub policy_rule_count: usize,
    pub route_rule_count: usize,
    pub capability_warnings: Vec<String>,
}

pub struct DomainPackPolicyValidator;

use std::collections::HashSet;

fn extract_declared_paths_from_toml(
    toml_str: &str,
) -> (HashSet<String>, HashSet<String>, HashSet<String>) {
    let mut policies = HashSet::new();
    let mut routes = HashSet::new();
    let mut schemas = HashSet::new();

    if let Ok(val) = toml::from_str::<serde_json::Value>(toml_str) {
        if let Some(arr) = val.get("policies").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(p) = item.get("path").and_then(|v| v.as_str()) {
                    policies.insert(p.to_string());
                }
            }
        }
        if let Some(arr) = val.get("routes").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(p) = item.get("path").and_then(|v| v.as_str()) {
                    routes.insert(p.to_string());
                }
            }
        }
        if let Some(arr) = val.get("schemas").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(p) = item.get("path").and_then(|v| v.as_str()) {
                    schemas.insert(p.to_string());
                }
            }
        }
    }
    (policies, routes, schemas)
}

impl DomainPackPolicyValidator {
    pub fn validate_bundle_policies(bundle: &DomainPackBundle) -> DomainPackValidationResult {
        let mut result = DomainPackValidationResult::default();

        let (declared_policies, declared_routes, declared_schemas) = bundle
            .files
            .get("pack.toml")
            .and_then(|c| std::str::from_utf8(c).ok())
            .map(extract_declared_paths_from_toml)
            .unwrap_or_default();

        for (rel_path, content) in &bundle.files {
            let lower_path = rel_path.to_lowercase();
            let is_policy = declared_policies.contains(rel_path)
                || (lower_path.contains("policy")
                    && (lower_path.ends_with(".toml") || lower_path.ends_with(".policy")));
            let is_route = declared_routes.contains(rel_path)
                || (lower_path.contains("route") && lower_path.ends_with(".toml"));
            let is_schema = declared_schemas.contains(rel_path)
                || (lower_path.ends_with(".json") && lower_path.contains("schema"));

            if is_policy {
                if let Ok(content_str) = std::str::from_utf8(content) {
                    match PolicySet::from_toml_str(content_str) {
                        Ok(policy_set) => {
                            result.policy_rule_count += policy_set.rules.len();
                        }
                        Err(e) => {
                            result
                                .syntax_errors
                                .push(format!("policy syntax error in {rel_path}: {e}"));
                        }
                    }
                } else {
                    result
                        .syntax_errors
                        .push(format!("non-UTF8 policy file in {rel_path}"));
                }
            } else if is_route {
                if let Ok(content_str) = std::str::from_utf8(content) {
                    match toml::from_str::<RouteTable>(content_str) {
                        Ok(route_table) => {
                            if let Err(e) = route_table.validate() {
                                result.syntax_errors.push(format!(
                                    "route table validation error in {rel_path}: {e}"
                                ));
                            } else {
                                result.route_rule_count += route_table.routes.len();
                            }
                        }
                        Err(e) => {
                            result
                                .syntax_errors
                                .push(format!("route syntax error in {rel_path}: {e}"));
                        }
                    }
                } else {
                    result
                        .syntax_errors
                        .push(format!("non-UTF8 route file in {rel_path}"));
                }
            } else if is_schema {
                if let Ok(content_str) = std::str::from_utf8(content) {
                    if let Err(e) = serde_json::from_str::<serde_json::Value>(content_str) {
                        result
                            .syntax_errors
                            .push(format!("invalid JSON schema in {rel_path}: {e}"));
                    }
                } else {
                    result
                        .syntax_errors
                        .push(format!("non-UTF8 schema file in {rel_path}"));
                }
            }
        }

        result.valid = result.syntax_errors.is_empty();
        result
    }

    pub fn validate_dir_policies(pack_dir: &Path) -> DomainPackValidationResult {
        let mut result = DomainPackValidationResult::default();

        let (declared_policies, declared_routes, declared_schemas) =
            fs::read_to_string(pack_dir.join("pack.toml"))
                .ok()
                .map(|s| extract_declared_paths_from_toml(&s))
                .unwrap_or_default();

        fn walk_and_validate(
            base_dir: &Path,
            current_dir: &Path,
            declared_policies: &HashSet<String>,
            declared_routes: &HashSet<String>,
            declared_schemas: &HashSet<String>,
            result: &mut DomainPackValidationResult,
        ) {
            let Ok(entries) = fs::read_dir(current_dir) else {
                result.syntax_errors.push(format!(
                    "failed to read directory {}",
                    current_dir.display()
                ));
                return;
            };

            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                if file_name.starts_with('.')
                    || file_name == "target"
                    || file_name == "node_modules"
                {
                    continue;
                }

                if path.is_dir() {
                    walk_and_validate(
                        base_dir,
                        &path,
                        declared_policies,
                        declared_routes,
                        declared_schemas,
                        result,
                    );
                } else if path.is_file() {
                    let rel_path = path
                        .strip_prefix(base_dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();
                    let lower_path = rel_path.to_lowercase();

                    let is_policy = declared_policies.contains(&rel_path)
                        || (lower_path.contains("policy")
                            && (lower_path.ends_with(".toml") || lower_path.ends_with(".policy")));
                    let is_route = declared_routes.contains(&rel_path)
                        || (lower_path.contains("route") && lower_path.ends_with(".toml"));
                    let is_schema = declared_schemas.contains(&rel_path)
                        || (lower_path.ends_with(".json") && lower_path.contains("schema"));

                    if is_policy {
                        if let Ok(content_str) = fs::read_to_string(&path) {
                            match PolicySet::from_toml_str(&content_str) {
                                Ok(policy_set) => {
                                    result.policy_rule_count += policy_set.rules.len();
                                }
                                Err(e) => {
                                    result
                                        .syntax_errors
                                        .push(format!("policy syntax error in {rel_path}: {e}"));
                                }
                            }
                        } else {
                            result
                                .syntax_errors
                                .push(format!("failed to read policy file {rel_path}"));
                        }
                    } else if is_route {
                        if let Ok(content_str) = fs::read_to_string(&path) {
                            match toml::from_str::<RouteTable>(&content_str) {
                                Ok(route_table) => {
                                    if let Err(e) = route_table.validate() {
                                        result.syntax_errors.push(format!(
                                            "route table validation error in {rel_path}: {e}"
                                        ));
                                    } else {
                                        result.route_rule_count += route_table.routes.len();
                                    }
                                }
                                Err(e) => {
                                    result
                                        .syntax_errors
                                        .push(format!("route syntax error in {rel_path}: {e}"));
                                }
                            }
                        } else {
                            result
                                .syntax_errors
                                .push(format!("failed to read route file {rel_path}"));
                        }
                    } else if is_schema {
                        if let Ok(content_str) = fs::read_to_string(&path) {
                            if let Err(e) = serde_json::from_str::<serde_json::Value>(&content_str)
                            {
                                result
                                    .syntax_errors
                                    .push(format!("invalid JSON schema in {rel_path}: {e}"));
                            }
                        } else {
                            result
                                .syntax_errors
                                .push(format!("failed to read schema file {rel_path}"));
                        }
                    }
                }
            }
        }

        walk_and_validate(
            pack_dir,
            pack_dir,
            &declared_policies,
            &declared_routes,
            &declared_schemas,
            &mut result,
        );
        result.valid = result.syntax_errors.is_empty();
        result
    }
}
