use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::{DomainPackRegistry, DomainPackRegistryEntry, DriverVersion, ZapStoreError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackDependencySpec {
    pub pack_id: String,
    pub version_req: String,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackResolutionPlan {
    pub target_pack: String,
    pub target_version: String,
    pub install_order: Vec<DomainPackRegistryEntry>,
    pub required_capabilities: Vec<String>,
    pub provided_capabilities: Vec<String>,
}

pub struct DomainPackDependencyResolver<'a> {
    pub store_registry: &'a DomainPackRegistry,
}

fn parse_version(s: &str) -> Option<DriverVersion> {
    let clean = s.trim().trim_start_matches('v');
    let parts: Vec<&str> = clean.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let major = parts[0].parse::<u64>().ok()?;
    let minor = parts[1].parse::<u64>().ok()?;
    let patch_str = parts[2].split('-').next().unwrap_or(parts[2]);
    let patch = patch_str.parse::<u64>().ok()?;
    Some(DriverVersion {
        major,
        minor,
        patch,
    })
}

pub fn matches_version_req(version_str: &str, req_str: &str) -> bool {
    let req_clean = req_str.trim();
    if req_clean == "*" || req_clean.is_empty() {
        return true;
    }

    let Some(v) = parse_version(version_str) else {
        return false;
    };

    if let Some(req_v) = req_clean.strip_prefix('^') {
        if let Some(target) = parse_version(req_v) {
            if target.major > 0 {
                return v.major == target.major && v >= target;
            } else if target.minor > 0 {
                return v.major == 0 && v.minor == target.minor && v >= target;
            } else {
                return v == target;
            }
        }
        return false;
    } else if let Some(req_v) = req_clean.strip_prefix(">=") {
        if let Some(target) = parse_version(req_v) {
            return v >= target;
        }
        return false;
    } else if let Some(req_v) = req_clean.strip_prefix('=') {
        if let Some(target) = parse_version(req_v) {
            return v == target;
        }
        return false;
    } else if let Some(target) = parse_version(req_clean) {
        return v == target;
    }

    false
}

impl<'a> DomainPackDependencyResolver<'a> {
    pub fn new(store_registry: &'a DomainPackRegistry) -> Self {
        Self { store_registry }
    }

    pub fn resolve(
        &self,
        target_id: &str,
        target_version: &str,
        dependencies: &[DomainPackDependencySpec],
    ) -> Result<DomainPackResolutionPlan, ZapStoreError> {
        let mut install_order = Vec::new();
        let mut visited_branch = HashSet::new();
        let mut resolved_ids = HashSet::new();

        visited_branch.insert(target_id.to_string());

        for dep in dependencies {
            self.resolve_dep(
                dep,
                &mut visited_branch,
                &mut resolved_ids,
                &mut install_order,
            )?;
        }

        let mut required_capabilities = Vec::new();
        let mut provided_capabilities = Vec::new();

        for entry in &install_order {
            for cap in &entry.compatibility.capabilities_required {
                if !required_capabilities.contains(cap) {
                    required_capabilities.push(cap.clone());
                }
            }
            for cap in &entry.compatibility.capabilities_provided {
                if !provided_capabilities.contains(cap) {
                    provided_capabilities.push(cap.clone());
                }
            }
        }

        Ok(DomainPackResolutionPlan {
            target_pack: target_id.to_string(),
            target_version: target_version.to_string(),
            install_order,
            required_capabilities,
            provided_capabilities,
        })
    }

    fn resolve_dep(
        &self,
        dep: &DomainPackDependencySpec,
        visited_branch: &mut HashSet<String>,
        resolved_ids: &mut HashSet<String>,
        install_order: &mut Vec<DomainPackRegistryEntry>,
    ) -> Result<(), ZapStoreError> {
        if resolved_ids.contains(&dep.pack_id) {
            return Ok(());
        }

        if visited_branch.contains(&dep.pack_id) {
            return Err(ZapStoreError::CircularDomainPackDependency(
                dep.pack_id.clone(),
            ));
        }

        let matching_entry = self
            .store_registry
            .entries
            .iter()
            .filter(|e| e.id == dep.pack_id)
            .find(|e| matches_version_req(&e.version, &dep.version_req));

        let entry = match matching_entry {
            Some(e) => e,
            None => {
                if dep.optional {
                    return Ok(());
                } else {
                    return Err(ZapStoreError::UnsatisfiedDomainPackDependency {
                        pack_id: dep.pack_id.clone(),
                        requirement: dep.version_req.clone(),
                    });
                }
            }
        };

        visited_branch.insert(dep.pack_id.clone());

        for sub_dep in &entry.dependencies {
            self.resolve_dep(sub_dep, visited_branch, resolved_ids, install_order)?;
        }

        visited_branch.remove(&dep.pack_id);
        resolved_ids.insert(dep.pack_id.clone());
        install_order.push(entry.clone());

        Ok(())
    }
}
