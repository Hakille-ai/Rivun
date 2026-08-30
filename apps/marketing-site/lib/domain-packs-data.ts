import { DomainPackInfo } from "./types";

export const DOMAIN_PACKS: DomainPackInfo[] = [
  {
    id: "rivun-pack-agentic-dev",
    name: "Agentic Dev Pack",
    tagline: "Autonomous Coding & DevOps Guardrails",
    category: "ai",
    version: "1.0.0",
    capabilitiesCount: 5,
    defaultSafetyGate: "Patch dry-run & automated test receipt verification",
    description:
      "Empowers autonomous coding agents to inspect repositories, generate patches, execute sandboxed tests, and draft PRs while enforcing strict workspace isolation and tamper-evident patch audit trails.",
    capabilities: [
      { name: "repo.read", description: "Read repository files, git trees, and commit metadata", risk: "low", requiredProof: "Ed25519 Session Signature" },
      { name: "test.run", description: "Execute test suites inside isolated WASM/container sandbox", risk: "medium", requiredProof: "Fuel-Metered Execution Receipt" },
      { name: "ci.inspect", description: "Query continuous integration build status and pipeline logs", risk: "low", requiredProof: "Read-Only Token Verification" },
      { name: "repo.patch", description: "Apply code modifications within permitted workspace directories", risk: "medium", requiredProof: "Policy Gate & Diff Review Hash" },
      { name: "pr.create", description: "Open pull requests and draft reviews with signed provenance", risk: "medium", requiredProof: "Attested Causal Provenance Chain" },
    ],
    manifestToml: `[pack]
name = "rivun-pack-agentic-dev"
version = "1.0.0"
description = "Autonomous coding and software delivery guardrails"
license = "Apache-2.0"
authors = ["Rivun Protocol Architects <security@rivun.org>"]

[dependencies]
"rivun-runtime" = ">=1.0.0"
"rivun-capability" = ">=1.0.0"

[capabilities]
"repo.read" = { level = "read_only", description = "Inspect source code" }
"repo.patch" = { level = "restricted", description = "Apply code diffs" }
"test.run" = { level = "sandboxed", description = "Execute test pipelines" }
"ci.inspect" = { level = "read_only", description = "Inspect CI runs" }
"pr.create" = { level = "collaborative", description = "Open pull requests" }`,
    policyToml: `[[rules]]
name = "enforce-workspace-boundary"
description = "Deny modifications outside the authorized repository root"
subject_pattern = "repo.patch"
decision = "allow"
condition = "ctx.path.starts_with(workspace.root) && !ctx.path.contains('/.git/')"

[[rules]]
name = "require-test-receipt-before-pr"
description = "Mandate a green test run receipt before opening a PR"
subject_pattern = "pr.create"
decision = "allow"
condition = "receipts.exists(type='test.run', status='passed', within_mins=15)"

[[rules]]
name = "default-fail-closed"
subject_pattern = "*"
decision = "deny"`,
    schemaJson: `{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "AgenticDevPatchRequest",
  "type": "object",
  "required": ["repo_url", "branch", "diff_patch", "commit_message"],
  "properties": {
    "repo_url": { "type": "string", "format": "uri" },
    "branch": { "type": "string" },
    "diff_patch": { "type": "string" },
    "commit_message": { "type": "string", "maxLength": 100 }
  }
}`,
  },
  {
    id: "rivun-pack-cloud-ops",
    name: "Cloud Ops Pack",
    tagline: "Infrastructure & Kubernetes Zero-Trust Automation",
    category: "cloud",
    version: "1.0.0",
    capabilitiesCount: 4,
    defaultSafetyGate: "Proof-of-Action (T=2) BFT Quorum & Canary Rollback Simulation",
    description:
      "Governs autonomous SRE agents managing cloud infrastructure, Kubernetes clusters, and IAM configurations with mandatory 2-phase BFT quorum on production environments.",
    capabilities: [
      { name: "infra.read", description: "Query cloud resource topology, metric counters, and IAM roles", risk: "low", requiredProof: "Identity Key Signature" },
      { name: "incident.escalate", description: "Trigger PagerDuty / OpsGenie on-call incident notifications", risk: "medium", requiredProof: "Anomaly Metric Evidence Hash" },
      { name: "deploy.rollout", description: "Initiate canary or blue-green container workload rollouts", risk: "high", requiredProof: "Proof-of-Action Quorum (T=2)" },
      { name: "infra.provision", description: "Execute Terraform / Pulumi mutating infrastructure changes", risk: "high", requiredProof: "Multi-Signature Operator Approval" },
    ],
    manifestToml: `[pack]
name = "rivun-pack-cloud-ops"
version = "1.0.0"
description = "Cloud infrastructure and Kubernetes operations governance"
license = "Apache-2.0"

[capabilities]
"infra.read" = { level = "read_only", description = "Query cluster state" }
"incident.escalate" = { level = "notification", description = "Trigger on-call alerts" }
"deploy.rollout" = { level = "mutating", description = "Canary deployment execution" }
"infra.provision" = { level = "critical", description = "Terraform mutations" }`,
    policyToml: `[[rules]]
name = "production-mutation-gate"
description = "Mandate Proof-of-Action consensus (T=2) for production namespaces"
subject_pattern = "deploy.rollout"
decision = "requires_poa"
condition = "ctx.environment == 'production'"
poa_threshold = 2

[[rules]]
name = "block-root-iam-creation"
description = "Block any agent attempt to grant administrative IAM roles"
subject_pattern = "infra.provision"
decision = "deny"
condition = "ctx.resource.type == 'iam_role' && ctx.role.admin == true"`,
    schemaJson: `{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "CloudOpsRolloutPayload",
  "type": "object",
  "required": ["cluster_id", "namespace", "deployment_name", "target_image"],
  "properties": {
    "cluster_id": { "type": "string" },
    "namespace": { "type": "string" },
    "deployment_name": { "type": "string" },
    "target_image": { "type": "string" },
    "canary_percentage": { "type": "integer", "minimum": 5, "maximum": 100 }
  }
}`,
  },
  {
    id: "rivun-pack-finance",
    name: "Finance & Settlement Pack",
    tagline: "Automated Trading, Escrows & Multi-Party PACTs",
    category: "enterprise",
    version: "1.0.0",
    capabilitiesCount: 4,
    defaultSafetyGate: "Double-entry balance validation & Multi-sig Escrow PACT",
    description:
      "Provides cryptographic guarantees for autonomous trading agents, liquidity rebalancers, and decentralized dispute resolution using deterministic PACT contracts.",
    capabilities: [
      { name: "quote.read", description: "Stream real-time orderbook feeds and depth charts", risk: "low", requiredProof: "Signed Feed Checksum" },
      { name: "risk.evaluate", description: "Compute Value-at-Risk (VaR) and counterparty margin ratios", risk: "low", requiredProof: "Deterministic Math Engine" },
      { name: "order.submit", description: "Route signed limit and market orders to liquidity pools", risk: "high", requiredProof: "Ed25519 Wallet Signature" },
      { name: "settlement.reconcile", description: "Execute atomic multi-party escrow release or dispute slashing", risk: "critical", requiredProof: "PACT Consensus + Arbitration Quorum" },
    ],
    manifestToml: `[pack]
name = "rivun-pack-finance"
version = "1.0.0"
description = "High-frequency trade execution and multi-party escrow settlements"
license = "Apache-2.0"

[capabilities]
"quote.read" = { level = "read_only" }
"risk.evaluate" = { level = "stateless_compute" }
"order.submit" = { level = "transactional" }
"settlement.reconcile" = { level = "consensus_mandatory" }`,
    policyToml: `[[rules]]
name = "max-notional-exposure-limit"
description = "Enforce hard cap on single-order transaction volume"
subject_pattern = "order.submit"
decision = "allow"
condition = "ctx.order.notional_usd <= 250000 && ctx.account.margin_ratio >= 1.5"

[[rules]]
name = "arbitration-slashing-rule"
description = "Require 3 of 4 arbitrator signatures to resolve disputed escrow"
subject_pattern = "settlement.reconcile"
decision = "requires_poa"
condition = "ctx.pact.state == 'disputed'"
poa_threshold = 3`,
    schemaJson: `{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "FinanceOrderSubmission",
  "type": "object",
  "required": ["symbol", "side", "amount", "price_limit", "pact_id"],
  "properties": {
    "symbol": { "type": "string" },
    "side": { "type": "string", "enum": ["BUY", "SELL"] },
    "amount": { "type": "number", "positive": true },
    "price_limit": { "type": "number" },
    "pact_id": { "type": "string", "format": "uuid" }
  }
}`,
  },
  {
    id: "rivun-pack-healthcare",
    name: "Healthcare & HIPAA Pack",
    tagline: "Clinical Coordination & Air-Gapped PHI Protection",
    category: "enterprise",
    version: "1.0.0",
    capabilitiesCount: 4,
    defaultSafetyGate: "Strict PHI redaction, HIPAA audit seal & Patient consent gate",
    description:
      "Enables clinical AI agents to coordinate patient care, parse electronic health records, and dispatch emergency alerts while guaranteeing HIPAA compliance and zero-knowledge data isolation.",
    capabilities: [
      { name: "consent.verify", description: "Validate patient electronic consent tokens against key registry", risk: "low", requiredProof: "Cryptographic Consent Hash" },
      { name: "records.read", description: "Access de-identified patient telemetry and clinical charts", risk: "medium", requiredProof: "Role-Based Key Token + Audit Record" },
      { name: "care.dispatch", description: "Dispatch nurse/physician clinical task orders", risk: "high", requiredProof: "Attending Physician Co-Signature" },
      { name: "audit.seal", description: "Generate immutable Merkle Mountain Range HIPAA compliance seals", risk: "critical", requiredProof: "MMR Root Batch Seal" },
    ],
    manifestToml: `[pack]
name = "rivun-pack-healthcare"
version = "1.0.0"
description = "Clinical AI care workflow with zero-trust PHI protection"
license = "Apache-2.0"

[compliance]
standards = ["HIPAA", "HITECH", "GDPR-Health", "SOC2-Type2"]`,
    policyToml: `[[rules]]
name = "enforce-phi-redaction"
description = "Deny any unencrypted or un-redacted PHI transmission"
subject_pattern = "records.read"
decision = "allow"
condition = "ctx.phi_scrubbed == true && ctx.consent_valid == true"

[[rules]]
name = "emergency-override-gate"
description = "Allow emergency triage dispatch with immediate break-glass audit seal"
subject_pattern = "care.dispatch"
decision = "allow"
condition = "ctx.triage_level == 'STAT_CRITICAL'"
action_hook = "audit.seal.immediate"`,
    schemaJson: `{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "ClinicalCareDispatch",
  "type": "object",
  "required": ["patient_hash", "care_protocol", "urgency", "consent_token"],
  "properties": {
    "patient_hash": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
    "care_protocol": { "type": "string" },
    "urgency": { "type": "string", "enum": ["ROUTINE", "URGENT", "STAT"] },
    "consent_token": { "type": "string" }
  }
}`,
  },
  {
    id: "rivun-pack-industrial",
    name: "Industrial & SCADA Pack",
    tagline: "Deterministic Edge Control & Modbus Safety Interlocks",
    category: "iot",
    version: "1.0.0",
    capabilitiesCount: 4,
    defaultSafetyGate: "Hardware interlock checks & PoA validator quorum",
    description:
      "Governs industrial edge controllers, Modbus/OPC-UA PLCs, and robotic arms with fuel-metered WASM drivers, microsecond-accurate SPSC ring-buffers, and hard real-time safety interlocks.",
    capabilities: [
      { name: "sensor.read", description: "Stream high-frequency vibration, thermal, and pressure registers", risk: "low", requiredProof: "Hardware Device Attestation" },
      { name: "plc.write", description: "Actuate Modbus coils and write holding registers", risk: "high", requiredProof: "Safety Interlock Check + Ed25519 Auth" },
      { name: "safety.override", description: "Perform temporary maintenance safety bypass with time-lock", risk: "critical", requiredProof: "Operator Dual-Key Authorization" },
      { name: "emergency.halt", description: "Execute immediate deterministic emergency E-Stop sequence", risk: "critical", requiredProof: "High-Priority Broadcast Frame" },
    ],
    manifestToml: `[pack]
name = "rivun-pack-industrial"
version = "1.0.0"
description = "SCADA, Modbus, and deterministic PLC safety orchestration"
license = "Apache-2.0"

[drivers]
modbus_bridge = "drivers/modbus_spsc.wasm"
safety_gate = "drivers/safety_interlock.wasm"`,
    policyToml: `[[rules]]
name = "thermal-safety-boundary"
description = "Prevent coil activation if temperature sensor exceeds threshold"
subject_pattern = "plc.write"
decision = "allow"
condition = "telemetry.get('temp_celsius') < 85.0 && telemetry.get('vibration_g') < 2.5"

[[rules]]
name = "emergency-halt-unrestricted"
description = "Emergency stop frames bypass normal queue with top priority"
subject_pattern = "emergency.halt"
decision = "allow"
flags_required = ["PRIORITY", "BROADCAST"]`,
    schemaJson: `{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "IndustrialPlcWriteCommand",
  "type": "object",
  "required": ["unit_id", "register_address", "value", "safety_token"],
  "properties": {
    "unit_id": { "type": "integer", "minimum": 1, "maximum": 247 },
    "register_address": { "type": "integer", "minimum": 0, "maximum": 65535 },
    "value": { "type": "integer" },
    "safety_token": { "type": "string" }
  }
}`,
  },
  {
    id: "rivun-pack-personal-ai",
    name: "Personal AI Assistant Pack",
    tagline: "Local Privacy, Secret Redaction & Sovereign Keys",
    category: "ai",
    version: "1.0.0",
    capabilitiesCount: 4,
    defaultSafetyGate: "User explicit biometric confirmation & spending limit gates",
    description:
      "Guarantees that personal AI agents operating on desktops and phones cannot leak credentials, execute unauthorized transactions, or send emails without explicit sovereign cryptographic gating.",
    capabilities: [
      { name: "calendar.read", description: "Read personal schedule and meeting invitations", risk: "low", requiredProof: "Local Vault Session Token" },
      { name: "email.draft", description: "Synthesize draft emails and summaries in staging sandbox", risk: "low", requiredProof: "Local Memory Receipt" },
      { name: "device.control", description: "Interact with local media, smart lights, and OS shortcuts", risk: "medium", requiredProof: "Desktop Operator Token" },
      { name: "purchase.authorize", description: "Execute financial payments or API subscriptions", risk: "high", requiredProof: "Biometric TouchID / Operator Vault Signature" },
    ],
    manifestToml: `[pack]
name = "rivun-pack-personal-ai"
version = "1.0.0"
description = "Personal agent privacy, local key vaults, and sovereign consent"
license = "Apache-2.0"`,
    policyToml: `[[rules]]
name = "spending-limit-threshold"
description = "Require explicit biometric confirmation for purchases over $50"
subject_pattern = "purchase.authorize"
decision = "allow"
condition = "ctx.amount_usd <= 50.00 || ctx.biometric_verified == true"

[[rules]]
name = "sandbox-unapproved-email-sending"
description = "Agents may only draft emails; outbound dispatch requires user click"
subject_pattern = "email.send"
decision = "deny"`,
    schemaJson: `{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "PersonalAiPurchaseAuthorization",
  "type": "object",
  "required": ["merchant", "amount", "currency", "item_description"],
  "properties": {
    "merchant": { "type": "string" },
    "amount": { "type": "number", "positive": true },
    "currency": { "type": "string", "maxLength": 3 },
    "item_description": { "type": "string" }
  }
}`,
  },
  {
    id: "rivun-pack-smart-building",
    name: "Smart Building & IoT Pack",
    tagline: "HVAC Optimization, Badge Access & Energy Mesh",
    category: "iot",
    version: "1.0.0",
    capabilitiesCount: 4,
    defaultSafetyGate: "Thermal safety envelope & Physical access audit logging",
    description:
      "Orchestrates autonomous building management agents managing zoned HVAC, automated badge access, and energy grid load shedding with complete tamper-evident audit logging.",
    capabilities: [
      { name: "telemetry.read", description: "Stream occupancy sensors, CO2 levels, and ambient temperatures", risk: "low", requiredProof: "IoT Sensor Attestation" },
      { name: "lighting.control", description: "Adjust daylight harvesting and LED luminance across zones", risk: "low", requiredProof: "Zone Policy Evaluation" },
      { name: "hvac.setpoint", description: "Modulate chiller setpoints and VAV damper positions", risk: "medium", requiredProof: "Energy Optimization Model Receipt" },
      { name: "badge.access", description: "Authorize biometric and NFC badge door unlocks", risk: "high", requiredProof: "Multi-Factor Access Verification Proof" },
    ],
    manifestToml: `[pack]
name = "rivun-pack-smart-building"
version = "1.0.0"
description = "Smart building HVAC, access control, and energy grid orchestration"
license = "Apache-2.0"`,
    policyToml: `[[rules]]
name = "hvac-comfort-envelope"
description = "Enforce temperature setpoint bounds between 19°C and 24°C"
subject_pattern = "hvac.setpoint"
decision = "allow"
condition = "ctx.target_temp >= 19.0 && ctx.target_temp <= 24.0"

[[rules]]
name = "after-hours-security-gate"
description = "Mandate 2-factor verification for physical badge unlocks after 20:00"
subject_pattern = "badge.access"
decision = "allow"
condition = "time.is_business_hours() || ctx.mfa_completed == true"`,
    schemaJson: `{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "SmartBuildingHvacSetpoint",
  "type": "object",
  "required": ["zone_id", "target_temperature", "fan_speed"],
  "properties": {
    "zone_id": { "type": "string" },
    "target_temperature": { "type": "number", "minimum": 16, "maximum": 28 },
    "fan_speed": { "type": "string", "enum": ["AUTO", "LOW", "MEDIUM", "HIGH"] }
  }
}`,
  },
];
