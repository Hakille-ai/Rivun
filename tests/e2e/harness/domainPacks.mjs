export const DOMAIN_PACKS = [
  {
    id: 'rivun-pack-agentic-dev',
    name: 'Agentic Development & DevOps',
    version: '1.0.0',
    description: 'Autonomous software engineering, patch application, and CI/CD verification.',
    capabilities: [
      { name: 'repo.read', risk: 'low' },
      { name: 'repo.patch', risk: 'medium' },
      { name: 'test.run', risk: 'medium' },
      { name: 'ci.inspect', risk: 'low' },
      { name: 'pr.create', risk: 'medium' },
    ],
    defaultSafetyGate: 'Patch dry-run & automated test receipt verification',
    memoryLimitMb: 128,
  },
  {
    id: 'rivun-pack-cloud-ops',
    name: 'Cloud Infrastructure & SRE',
    version: '1.0.0',
    description: 'Infrastructure provisioning, canary rollouts, and incident escalation.',
    capabilities: [
      { name: 'infra.read', risk: 'low' },
      { name: 'infra.provision', risk: 'high' },
      { name: 'deploy.rollout', risk: 'high' },
      { name: 'incident.escalate', risk: 'medium' },
    ],
    defaultSafetyGate: 'Human approval & canary rollback simulation',
    memoryLimitMb: 256,
  },
  {
    id: 'rivun-pack-finance',
    name: 'Automated Trading & Settlement',
    version: '1.0.0',
    description: 'Real-time order submission, risk evaluation, and multi-sig settlement reconciliation.',
    capabilities: [
      { name: 'quote.read', risk: 'low' },
      { name: 'risk.evaluate', risk: 'low' },
      { name: 'order.submit', risk: 'high' },
      { name: 'settlement.reconcile', risk: 'critical' },
    ],
    defaultSafetyGate: 'Double-entry balance check & multi-sig PoA',
    memoryLimitMb: 512,
  },
  {
    id: 'rivun-pack-healthcare',
    name: 'Clinical Care Coordination',
    version: '1.0.0',
    description: 'HIPAA-compliant EHR data access, consent verification, and care dispatch.',
    capabilities: [
      { name: 'records.read', risk: 'medium' },
      { name: 'consent.verify', risk: 'low' },
      { name: 'care.dispatch', risk: 'high' },
      { name: 'audit.seal', risk: 'critical' },
    ],
    defaultSafetyGate: 'Strict PHI redaction, HIPAA audit seal, consent gate',
    memoryLimitMb: 256,
  },
  {
    id: 'rivun-pack-industrial',
    name: 'SCADA & Industrial Edge Control',
    version: '1.0.0',
    description: 'Modbus/PLC sensor streams, safety overrides, and emergency halts.',
    capabilities: [
      { name: 'sensor.read', risk: 'low' },
      { name: 'plc.write', risk: 'high' },
      { name: 'safety.override', risk: 'critical' },
      { name: 'emergency.halt', risk: 'critical' },
    ],
    defaultSafetyGate: 'Hardware interlock checks & PoA validator quorum',
    memoryLimitMb: 64,
  },
  {
    id: 'rivun-pack-personal-ai',
    name: 'Personal Assistant & Device Automation',
    version: '1.0.0',
    description: 'Email drafting, calendar scheduling, purchases, and smart device control.',
    capabilities: [
      { name: 'calendar.read', risk: 'low' },
      { name: 'email.draft', risk: 'low' },
      { name: 'purchase.authorize', risk: 'high' },
      { name: 'device.control', risk: 'medium' },
    ],
    defaultSafetyGate: 'User explicit consent & spending limit gates',
    memoryLimitMb: 128,
  },
  {
    id: 'rivun-pack-smart-building',
    name: 'Smart Building IoT & Energy Management',
    version: '1.0.0',
    description: 'HVAC setpoint tuning, badge access verification, and lighting telemetry.',
    capabilities: [
      { name: 'telemetry.read', risk: 'low' },
      { name: 'hvac.setpoint', risk: 'medium' },
      { name: 'badge.access', risk: 'high' },
      { name: 'lighting.control', risk: 'low' },
    ],
    defaultSafetyGate: 'Thermal safety envelope & physical access logs',
    memoryLimitMb: 64,
  },
];

export function getDomainPack(packId) {
  return DOMAIN_PACKS.find((p) => p.id === packId || p.id === 'rivun-pack-' + packId);
}

export function generateInstallCommand(packId) {
  const pack = getDomainPack(packId);
  if (!pack) throw new Error('Unknown pack: ' + packId);
  return 'rivun pack install ' + pack.id + '@' + pack.version + ' --verify-signature';
}

export function validatePackManifest(manifest) {
  if (!manifest || !manifest.id || !manifest.version) {
    return { valid: false, error: 'Missing id or version' };
  }
  if (!Array.isArray(manifest.capabilities) || manifest.capabilities.length === 0) {
    return { valid: false, error: 'Pack must declare at least one capability' };
  }
  const validRisks = new Set(['low', 'medium', 'high', 'critical']);
  for (const cap of manifest.capabilities) {
    if (!cap.name || !validRisks.has(cap.risk)) {
      return { valid: false, error: 'Invalid capability format or unknown risk: ' + cap.risk };
    }
  }
  return { valid: true };
}
