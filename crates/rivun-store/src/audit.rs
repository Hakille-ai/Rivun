use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::bundle::DomainPackBundle;
use crate::{DomainPackRisk, DomainPackStatus, RivunStoreError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditIssue {
    pub severity: DomainPackRisk,
    pub category: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackAuditReport {
    pub pack_id: String,
    pub version: String,
    pub overall_risk: DomainPackRisk,
    pub max_risk_allowed: DomainPackRisk,
    pub passed: bool,
    pub issues: Vec<AuditIssue>,
}

pub fn audit_pack_dir(
    pack_dir: &Path,
    max_risk: Option<DomainPackRisk>,
) -> Result<PackAuditReport, RivunStoreError> {
    let manifest_path = pack_dir.join("pack.toml");
    if !manifest_path.exists() {
        return Err(RivunStoreError::InvalidDomainPackBundleFormat(format!(
            "missing pack.toml in {}",
            pack_dir.display()
        )));
    }

    let manifest_str =
        fs::read_to_string(&manifest_path).map_err(|e| RivunStoreError::IoError(e.to_string()))?;
    let pack_toml: serde_json::Value = toml::from_str(&manifest_str)
        .map_err(|e| RivunStoreError::InvalidDomainPackBundleFormat(e.to_string()))?;

    let pack_id = pack_toml
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let version = pack_toml
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.1.0")
        .to_string();

    let mut issues = Vec::new();
    let mut highest_risk = DomainPackRisk::Low;

    let status_str = pack_toml
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("active");
    match status_str.to_lowercase().as_str() {
        "deprecated" => {
            issues.push(AuditIssue {
                severity: DomainPackRisk::Medium,
                category: "status".to_string(),
                message: "domain pack status is deprecated".to_string(),
            });
            if DomainPackRisk::Medium > highest_risk {
                highest_risk = DomainPackRisk::Medium;
            }
        }
        "revoked" => {
            issues.push(AuditIssue {
                severity: DomainPackRisk::Critical,
                category: "status".to_string(),
                message: "domain pack status is revoked".to_string(),
            });
            highest_risk = DomainPackRisk::Critical;
        }
        _ => {}
    }

    if let Some(caps) = pack_toml.get("capabilities").and_then(|v| v.as_array()) {
        for cap in caps {
            let cap_id = cap.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let risk_str = cap.get("risk").and_then(|v| v.as_str()).unwrap_or("low");
            let risk = match risk_str.to_lowercase().as_str() {
                "critical" => DomainPackRisk::Critical,
                "high" => DomainPackRisk::High,
                "medium" => DomainPackRisk::Medium,
                _ => DomainPackRisk::Low,
            };

            if risk > highest_risk {
                highest_risk = risk;
            }

            if risk == DomainPackRisk::Critical {
                issues.push(AuditIssue {
                    severity: DomainPackRisk::Critical,
                    category: "capabilities".to_string(),
                    message: format!("critical risk capability `{cap_id}` granted"),
                });
            } else if risk == DomainPackRisk::High {
                issues.push(AuditIssue {
                    severity: DomainPackRisk::High,
                    category: "capabilities".to_string(),
                    message: format!("high risk capability `{cap_id}` granted"),
                });
            }
        }
    }

    let max_allowed = max_risk.unwrap_or(DomainPackRisk::Critical);
    let passed = highest_risk <= max_allowed;

    Ok(PackAuditReport {
        pack_id,
        version,
        overall_risk: highest_risk,
        max_risk_allowed: max_allowed,
        passed,
        issues,
    })
}

pub fn audit_bundle(
    bundle: &DomainPackBundle,
    max_risk: Option<DomainPackRisk>,
) -> Result<PackAuditReport, RivunStoreError> {
    let mut issues = Vec::new();
    let mut highest_risk = DomainPackRisk::Low;

    if bundle.manifest.status == DomainPackStatus::Deprecated {
        issues.push(AuditIssue {
            severity: DomainPackRisk::Medium,
            category: "status".to_string(),
            message: "domain pack status is deprecated".to_string(),
        });
    } else if bundle.manifest.status == DomainPackStatus::Revoked {
        issues.push(AuditIssue {
            severity: DomainPackRisk::Critical,
            category: "status".to_string(),
            message: "domain pack status is revoked".to_string(),
        });
        highest_risk = DomainPackRisk::Critical;
    }

    if let Some(content) = bundle.files.get("pack.toml")
        && let Ok(str_val) = std::str::from_utf8(content)
        && let Ok(pack_toml) = toml::from_str::<serde_json::Value>(str_val)
        && let Some(caps) = pack_toml.get("capabilities").and_then(|v| v.as_array())
    {
        for cap in caps {
            let cap_id = cap.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let risk_str = cap.get("risk").and_then(|v| v.as_str()).unwrap_or("low");
            let risk = match risk_str.to_lowercase().as_str() {
                "critical" => DomainPackRisk::Critical,
                "high" => DomainPackRisk::High,
                "medium" => DomainPackRisk::Medium,
                _ => DomainPackRisk::Low,
            };

            if risk > highest_risk {
                highest_risk = risk;
            }

            if risk >= DomainPackRisk::High {
                issues.push(AuditIssue {
                    severity: risk,
                    category: "capabilities".to_string(),
                    message: format!("{risk_str} risk capability `{cap_id}` granted"),
                });
            }
        }
    }

    let max_allowed = max_risk.unwrap_or(DomainPackRisk::Critical);
    let passed = highest_risk <= max_allowed;

    Ok(PackAuditReport {
        pack_id: bundle.manifest.pack_id.clone(),
        version: bundle.manifest.version.clone(),
        overall_risk: highest_risk,
        max_risk_allowed: max_allowed,
        passed,
        issues,
    })
}
