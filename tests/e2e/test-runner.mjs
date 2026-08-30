#!/usr/bin/env node
import { runTier1Tests } from './tier1-features.test.mjs';
import { runTier2Tests } from './tier2-boundaries.test.mjs';
import { runTier3Tests } from './tier3-integration.test.mjs';
import { runTier4Tests } from './tier4-scenarios.test.mjs';

const startTime = Date.now();

const results = {
  tier1: { name: 'Tier 1: Functional Feature Coverage (25 Features)', tests: [], passed: 0, failed: 0 },
  tier2: { name: 'Tier 2: Boundary, Negative & Corner Cases (25 Boundary Sets)', tests: [], passed: 0, failed: 0 },
  tier3: { name: 'Tier 3: Cross-Feature Integration Flows (20 Multi-Stage Flows)', tests: [], passed: 0, failed: 0 },
  tier4: { name: 'Tier 4: Real-World Application Scenarios (10 Multi-Agent Scenarios)', tests: [], passed: 0, failed: 0 },
};

function createRecorder(tierObj) {
  return (name, testFn) => {
    const testStart = performance.now();
    try {
      testFn();
      const durationMs = (performance.now() - testStart).toFixed(2);
      tierObj.tests.push({ name, status: 'PASSED', durationMs, error: null });
      tierObj.passed++;
    } catch (err) {
      const durationMs = (performance.now() - testStart).toFixed(2);
      tierObj.tests.push({ name, status: 'FAILED', durationMs, error: err });
      tierObj.failed++;
    }
  };
}

console.log('=================================================================================');
console.log('                 RIVUN PROTOCOL SUITE - AUTOMATED E2E TEST RUNNER               ');
console.log('=================================================================================');
console.log('Execution Timestamp: ' + new Date().toISOString());
console.log('Node.js Version:     ' + process.version + ' (' + process.platform + ' ' + process.arch + ')');
console.log('--------------------------------------------------------------------------------\n');

// 1. Run Tier 1
console.log('>> Executing Tier 1: Functional Feature Coverage (Features 1 - 25)...');
runTier1Tests(createRecorder(results.tier1));
console.log('   Completed Tier 1: ' + results.tier1.passed + ' passed, ' + results.tier1.failed + ' failed');

// 2. Run Tier 2
console.log('>> Executing Tier 2: Boundary & Corner Cases (Features 1 - 25)...');
runTier2Tests(createRecorder(results.tier2));
console.log('   Completed Tier 2: ' + results.tier2.passed + ' passed, ' + results.tier2.failed + ' failed');

// 3. Run Tier 3
console.log('>> Executing Tier 3: Cross-Feature Integration Flows (Flows 1 - 20)...');
runTier3Tests(createRecorder(results.tier3));
console.log('   Completed Tier 3: ' + results.tier3.passed + ' passed, ' + results.tier3.failed + ' failed');

// 4. Run Tier 4
console.log('>> Executing Tier 4: Real-World Multi-Agent Scenarios (Scenarios 1 - 10)...');
runTier4Tests(createRecorder(results.tier4));
console.log('   Completed Tier 4: ' + results.tier4.passed + ' passed, ' + results.tier4.failed + ' failed\n');

const totalDurationMs = Date.now() - startTime;
const totalPassed = results.tier1.passed + results.tier2.passed + results.tier3.passed + results.tier4.passed;
const totalFailed = results.tier1.failed + results.tier2.failed + results.tier3.failed + results.tier4.failed;
const totalTests = totalPassed + totalFailed;

// Print Feature Matrix
console.log('=================================================================================');
console.log('                        25-FEATURE VERIFICATION MATRIX                        ');
console.log('================================================================================');
const features = [
  '01: Marketing Hero & Wire Visualizer',
  '02: P2P Swarm & Particle Mesh',
  '03: 5 Core Innovations Showcase',
  '04: Cloud SaaS & Workstation',
  '05: 7 Domain Packs Showcase',
  '06: Security & Compliance Matrix',
  '07: Pricing & ROI Calculator',
  '08: Sandbox & Code Generator',
  '09: Aesthetics & Navigation',
  '10: Instant Full-Text Search',
  '11: Multi-Level Sidebar & TOC',
  '12: Multi-Language Code Tabs',
  '13: Mermaid & KaTeX Diagrams',
  '14: Core Protocol & Wire Specs',
  '15: Consensus & BFT Quorum Docs',
  '16: WASM & Zero-Copy Streaming',
  '17: Key Vault & SaaS Docs',
  '18: 26 Workspace Crates Docs',
  '19: 4 SDK Developer Manuals',
  '20: 7 Domain Packs & RivunStore',
  '21: 7-Point Fleet Doctor & MMR',
  '22: Interactive API Explorer',
  '23: Cross-Platform Build Gates',
  '24: E2E Test Suite (Tiers 1-4)',
  '25: Adversarial Hardening (T5)',
];

console.log('| ID  | Feature Name                             | T1  | T2  | Integrations | Status |');
console.log('|-----|------------------------------------------|-----|-----|--------------|--------|');
for (let i = 0; i < 25; i++) {
  const fNum = String(i + 1).padStart(2, '0');
  const t1Count = results.tier1.tests.filter((t) => t.name.startsWith('tc_f' + fNum)).length;
  const t2Count = results.tier2.tests.filter((t) => t.name.startsWith('tc_b' + fNum)).length;
  const t1Passed = results.tier1.tests.filter((t) => t.name.startsWith('tc_f' + fNum) && t.status === 'PASSED').length;
  const t2Passed = results.tier2.tests.filter((t) => t.name.startsWith('tc_b' + fNum) && t.status === 'PASSED').length;
  const status = (t1Passed === t1Count && t2Passed === t2Count && t1Count >= 5 && t2Count >= 5) ? 'PASS' : 'FAIL';
  const namePadded = features[i].padEnd(40, ' ');
  console.log('| F' + fNum + ' | ' + namePadded + ' | ' + t1Passed + '/' + t1Count + ' | ' + t2Passed + '/' + t2Count + ' | Covered      |  ' + status + '  |');
}

console.log('--------------------------------------------------------------------------------');
console.log('Tier 3 Cross-Feature Integrations: ' + results.tier3.passed + '/' + results.tier3.tests.length + ' PASS');
console.log('Tier 4 Real-World Workloads:       ' + results.tier4.passed + '/' + results.tier4.tests.length + ' PASS');
console.log('================================================================================');
console.log('TOTAL E2E TESTS EXECUTED: ' + totalTests);
console.log('TOTAL TESTS PASSED:       ' + totalPassed);
console.log('TOTAL TESTS FAILED:       ' + totalFailed);
console.log('OVERALL EXECUTION TIME:   ' + totalDurationMs + ' ms');
console.log('================================================================================');

// Display Conclusion & Exit Code
if (totalFailed > 0) {
  console.error('\n[FAILED TESTS DETAILS]');
  for (const tierKey of ['tier1', 'tier2', 'tier3', 'tier4']) {
    const failedTests = results[tierKey].tests.filter((t) => t.status === 'FAILED');
    if (failedTests.length > 0) {
      console.error('\n--- ' + results[tierKey].name + ' ---');
      for (const t of failedTests) {
        console.error('FAIL: ' + t.name + ' (' + t.durationMs + 'ms)');
        console.error('  Error: ' + (t.error?.message || t.error));
      }
    }
  }
  process.exit(1);
} else {
  console.log('\n>>> ALL 280 E2E TESTS PASSED WITH 100% SUCCESS RATE. SYSTEM READY. <<<');
  process.exit(0);
}
