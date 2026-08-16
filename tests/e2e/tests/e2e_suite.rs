//! ZAP Next-Gen Frontier Comprehensive End-to-End Test Suite (`e2e_suite.rs`)
//!
//! Master test suite orchestrating 4 tiers of opaque-box, requirement-driven tests
//! across all 15 features in `PROJECT.md § Feature Inventory`.
//!
//! - **Tier 1**: Feature Coverage (75 tests across Features 1-15)
//! - **Tier 2**: Boundary, Corner Cases & Negative Tests (75 tests across Features 1-15)
//! - **Tier 3**: Cross-Feature Interactions & Combinations (15 tests)
//! - **Tier 4**: Real-World Multi-Agent Workload Scenarios (8 tests)
//! Total E2E Tests: 173+ tests.

pub mod tier1_feature_tests;
pub mod tier2_boundary_tests;
pub mod tier3_combination_tests;
pub mod tier4_realworld_tests;

use zap_e2e::harness::*;

#[test]
fn tc_e2e_suite_sanity_check() {
    assert_eq!(zap_e2e::e2e_harness_version(), "0.1.0");
    let key = generate_keypair();
    assert!(!public_key_string(&key).is_empty());
}
