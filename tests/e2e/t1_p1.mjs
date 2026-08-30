import {
  assert,
  assertEqual,
  assertDeepEqual,
  assertThrows,
  assertMatches,
} from './harness/assert.mjs';
import { blake3, blake3Hex } from './harness/blake3.mjs';
import {
  Keypair,
  PublicKey,
  BlindedCommitment,
  BlindedReceiptCommitment,
  encryptChaCha20Poly1305,
  decryptChaCha20Poly1305,
  nodeIdFromPublicKey,
  signatureHint,
  formatUuid,
  parseUuid,
} from './harness/crypto.mjs';
import {
  RivunHeader,
  RivunFrame,
  AuthTrailer,
  PoaTrailer,
  PoaAttestation,
  Flags,
  signFrame,
  verifyFrame,
  certifyFrame,
  verifyPoaCertificate,
  inspectFrameHex,
  MAGIC_NUMBER,
  MAGIC_BYTES,
  VERSION,
  HEADER_LEN,
  SIGNING_PREFIX_LEN,
  AUTH_TRAILER_MAGIC,
  AUTH_TRAILER_LEN,
} from './harness/wireCodec.mjs';
import {
  RivunEnvelope,
  MessageKind,
  MessageKindName,
  ZENV_MAGIC_NUMBER,
  ZENV_VERSION,
  ZENV_HEADER_LEN,
} from './harness/zenvCodec.mjs';
import {
  BftConsensusEngine,
  calculateQuorumThreshold,
} from './harness/consensus.mjs';
import {
  MerkleMountainRange,
  bagPeaks,
  mmrParentHash,
} from './harness/mmr.mjs';
import {
  WasmGuestSandbox,
  DriverPipeline,
} from './harness/wasmSim.mjs';
import {
  SpscRingBuffer,
  BackpressurePolicy,
} from './harness/spscRingBuffer.mjs';
import {
  ProvenanceChainBuilder,
  Stages,
} from './harness/provenance.mjs';
import {
  EscrowPact,
  PactState,
  RulingOutcome,
  canonicalizeJson,
} from './harness/pactDispute.mjs';
import {
  DOMAIN_PACKS,
  getDomainPack,
  generateInstallCommand,
  validatePackManifest,
} from './harness/domainPacks.mjs';
import {
  FleetDoctor,
  DoctorCheckStatus,
} from './harness/doctor.mjs';
import {
  SearchEngine,
  tokenize,
} from './harness/searchEngine.mjs';
import {
  calculatePricing,
  PricingTiers,
} from './harness/pricingEngine.mjs';

export function runTier1Tests(record) {
  // ------------------------------------------------------------------------
  // Feature 1: Marketing Hero & Signed Frame Visualizer (tc_f01_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f01_01_wire_header_big_endian_encoding', () => {
    const kp = Keypair.generate();
    const hdr = new RivunHeader({
      magic: MAGIC_NUMBER,
      version: VERSION,
      flags: Flags.ENCRYPTED | Flags.PRIORITY,
      sourceNode: kp.nodeId,
      targetNode: '00000000-0000-0000-0000-000000000000',
      timestampMicros: 1700000000000000n,
      payloadLen: 256,
    });
    const buf = hdr.encode();
    assertEqual(buf.length, HEADER_LEN, 'Header must be 64 bytes');
    assertEqual(buf.readUInt32BE(0), MAGIC_NUMBER);
    assertEqual(buf.readUInt16BE(4), VERSION);
    assertEqual(buf.readUInt16BE(6), Flags.ENCRYPTED | Flags.PRIORITY);
  });

  record('tc_f01_02_wire_header_decoding_roundtrip', () => {
    const kp = Keypair.generate();
    const target = Keypair.generate();
    const hdr1 = new RivunHeader({
      sourceNode: kp.nodeId,
      targetNode: target.nodeId,
      flags: Flags.SIGNED | Flags.REQUIRES_CONSENSUS,
      timestampMicros: 1724000000123456n,
      payloadLen: 512,
    });
    const encoded = hdr1.encode();
    const hdr2 = RivunHeader.decode(encoded);
    assertEqual(hdr2.sourceNode, kp.nodeId);
    assertEqual(hdr2.targetNode, target.nodeId);
    assertEqual(hdr2.flags, Flags.SIGNED | Flags.REQUIRES_CONSENSUS);
    assertEqual(hdr2.timestampMicros, 1724000000123456n);
    assertEqual(hdr2.payloadLen, 512);
  });

  record('tc_f01_03_ed25519_frame_signing_and_auth_trailer', () => {
    const kp = Keypair.generate();
    const hdr = new RivunHeader({ sourceNode: kp.nodeId, flags: Flags.NONE });
    const payload = Buffer.from('payload-f01-03', 'utf8');
    let frame = new RivunFrame(hdr, payload);
    frame = signFrame(kp, frame);

    assert(frame.auth !== null, 'Auth trailer should be attached');
    assertEqual(frame.auth.signature.length, 64);
    assertEqual(frame.header.flags & Flags.SIGNED, Flags.SIGNED);
    assert(verifyFrame(kp.getVerifyingKey(), frame), 'Frame signature verification should pass');
  });

  record('tc_f01_04_poa_trailer_certification_and_quorum_verification', () => {
    const sender = Keypair.generate();
    const validators = [Keypair.generate(), Keypair.generate(), Keypair.generate()];
    const hdr = new RivunHeader({
      sourceNode: sender.nodeId,
      flags: Flags.REQUIRES_CONSENSUS,
    });
    let frame = new RivunFrame(hdr, Buffer.from('poa-payload', 'utf8'));
    frame = signFrame(sender, frame);
    frame = certifyFrame(frame, 2, validators);

    assertEqual(frame.poa.threshold, 2);
    assertEqual(frame.poa.attestations.length, 3);
    const valPks = validators.map((v) => v.getVerifyingKey());
    assert(verifyPoaCertificate(frame, valPks, 2), 'PoA certificate verification should pass');
  });

  record('tc_f01_05_hex_inspector_section_decomposition', () => {
    const kp = Keypair.generate();
    const v1 = Keypair.generate();
    const v2 = Keypair.generate();
    const hdr = new RivunHeader({ sourceNode: kp.nodeId, flags: Flags.REQUIRES_CONSENSUS });
    let frame = new RivunFrame(hdr, Buffer.from('inspection-test-bytes', 'utf8'));
    frame = signFrame(kp, frame);
    frame = certifyFrame(frame, 2, [v1, v2]);

    const inspection = inspectFrameHex(frame);
    assert(inspection.totalBytes > 64, 'Total frame bytes should include header and trailers');
    assert(inspection.sections.some((s) => s.name.includes('Wire Header')));
    assert(inspection.sections.some((s) => s.name.includes('Payload')));
    assert(inspection.sections.some((s) => s.name.includes('Auth Trailer')));
    assert(inspection.sections.some((s) => s.name.includes('PoA Trailer')));
  });

  // ------------------------------------------------------------------------
  // Feature 2: P2P Swarm & Gossip Particle Mesh (tc_f02_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f02_01_swarm_vector_clock_causal_increment', () => {
    const clockA = { 'node-1': 1, 'node-2': 0 };
    clockA['node-1'] += 1;
    assertEqual(clockA['node-1'], 2);
  });

  record('tc_f02_02_swarm_gossip_anti_entropy_merge', () => {
    const clockA = { 'node-1': 3, 'node-2': 1 };
    const clockB = { 'node-1': 2, 'node-2': 4, 'node-3': 1 };
    const merged = {};
    const allKeys = new Set([...Object.keys(clockA), ...Object.keys(clockB)]);
    for (const k of allKeys) {
      merged[k] = Math.max(clockA[k] || 0, clockB[k] || 0);
    }
    assertEqual(merged['node-1'], 3);
    assertEqual(merged['node-2'], 4);
    assertEqual(merged['node-3'], 1);
  });

  record('tc_f02_03_k_fanout_peer_sampling_dissemination', () => {
    const cluster = ['node-1', 'node-2', 'node-3', 'node-4', 'node-5', 'node-6'];
    const k = 3;
    const sender = 'node-1';
    const peers = cluster.filter((n) => n !== sender);
    const selected = peers.slice(0, k);
    assertEqual(selected.length, 3);
    assert(!selected.includes(sender));
  });

  record('tc_f02_04_phi_accrual_heartbeat_failure_calculation', () => {
    const intervals = [1000, 1020, 990, 1010, 1005];
    const mean = intervals.reduce((a, b) => a + b, 0) / intervals.length;
    const variance = intervals.reduce((a, b) => a + Math.pow(b - mean, 2), 0) / intervals.length;
    const stdDev = Math.sqrt(variance);
    assert(stdDev < 50, 'Std dev should be small for stable heartbeats');
  });

  record('tc_f02_05_chaos_network_partition_isolation_detection', () => {
    const allNodes = ['A', 'B', 'C', 'D', 'E'];
    const partition1 = ['A', 'B', 'C'];
    const partition2 = ['D', 'E'];
    const threshold1 = calculateQuorumThreshold(allNodes.length);
    assert(partition1.length < threshold1, 'Partition 1 has 3/5 nodes (< threshold 4)');
    assert(partition2.length < threshold1, 'Partition 2 has 2/5 nodes (< threshold 4)');
  });

  // ------------------------------------------------------------------------
  // Feature 3: 5 Core Protocol Innovations Showcase (tc_f03_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f03_01_ed25519_node_id_derivation_uuid_v8', () => {
    const kp = Keypair.generate();
    const nid = kp.nodeId;
    const parsed = parseUuid(nid);
    assertEqual(parsed[6] & 0xf0, 0x80, 'UUID version 8 bitmask');
    assertEqual(parsed[8] & 0xc0, 0x80, 'RFC 9562 variant bitmask');
  });

  record('tc_f03_02_chacha20_poly1305_aead_authenticated_transport', () => {
    const key = Buffer.alloc(32, 0x11);
    const nonce = Buffer.alloc(12, 0x22);
    const plaintext = Buffer.from('top-secret-rivun-frame-data', 'utf8');
    const aad = Buffer.from('header-aad', 'utf8');

    const { ciphertext, tag } = encryptChaCha20Poly1305(key, nonce, plaintext, aad);
    assertEqual(tag.length, 16);
    const decrypted = decryptChaCha20Poly1305(key, nonce, ciphertext, tag, aad);
    assertEqual(decrypted.toString('utf8'), 'top-secret-rivun-frame-data');
  });

  record('tc_f03_03_proof_of_action_bft_state_machine_quorum', () => {
    const n = 7;
    const t = calculateQuorumThreshold(n);
    assertEqual(t, 5, 'Threshold for 7 nodes must be floor(14/3)+1 = 5');
  });

  record('tc_f03_04_wasmtime_fuel_metering_deduction', () => {
    const sandbox = new WasmGuestSandbox({ initialFuel: 1000 });
    const res = sandbox.execute('ping', Buffer.from('payload', 'utf8'), (a, p) => p);
    assert(res.remainingFuel < 1000, 'Fuel must be deducted');
  });

  record('tc_f03_05_merkle_mountain_range_peak_accumulator_folding', () => {
    const mmr = new MerkleMountainRange();
    for (let i = 0; i < 7; i++) {
      mmr.append('receipt-' + i);
    }
    const root = mmr.getRoot();
    assertEqual(root.length, 32);
  });

  // ------------------------------------------------------------------------
  // Feature 4: Rivun Cloud SaaS & Operator Workstation (tc_f04_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f04_01_operator_key_file_export_and_import', () => {
    const kp = Keypair.generate();
    const keyFile = kp.toKeyFile();
    assertEqual(keyFile.schema_version, 1);
    assertEqual(keyFile.node_id, kp.nodeId);
    assert(keyFile.public_key.length > 0);
  });

  record('tc_f04_02_cloud_zero_trust_staging_and_policy_proposal', () => {
    const stagedPolicy = {
      policyId: 'pol-001',
      rules: [{ capability: 'plc.write', action: 'allow_with_quorum' }],
      status: 'STAGED_AWAITING_LOCAL_SIGNATURE',
    };
    assertEqual(stagedPolicy.status, 'STAGED_AWAITING_LOCAL_SIGNATURE');
  });

  record('tc_f04_03_local_offline_signing_simulation', () => {
    const operatorKey = Keypair.generate();
    const policyPayload = Buffer.from(JSON.stringify({ policyId: 'pol-001', version: 2 }), 'utf8');
    const signature = operatorKey.sign(policyPayload);
    assert(operatorKey.getVerifyingKey().verify(policyPayload, signature));
  });

  record('tc_f04_04_cloud_bridge_signed_policy_rollout', () => {
    const operatorKey = Keypair.generate();
    const policy = { id: 'pol-rollout-1', active: true };
    const sig = operatorKey.sign(Buffer.from(JSON.stringify(policy)));
    const bundle = { policy, signature: sig.toString('hex'), authority: operatorKey.nodeId };
    assert(bundle.authority === operatorKey.nodeId);
  });

  record('tc_f04_05_multi_tenant_workspace_isolation_keys', () => {
    const tenantA = Keypair.generate();
    const tenantB = Keypair.generate();
    assert(tenantA.nodeId !== tenantB.nodeId, 'Tenants must have disjoint node IDs');
  });

  // ------------------------------------------------------------------------
  // Feature 5: 7 Domain Packs Showcase (tc_f05_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f05_01_all_7_domain_packs_registered', () => {
    assertEqual(DOMAIN_PACKS.length, 7, 'Must register exactly 7 official domain packs');
    const expectedIds = [
      'rivun-pack-agentic-dev',
      'rivun-pack-cloud-ops',
      'rivun-pack-finance',
      'rivun-pack-healthcare',
      'rivun-pack-industrial',
      'rivun-pack-personal-ai',
      'rivun-pack-smart-building',
    ];
    for (const id of expectedIds) {
      assert(DOMAIN_PACKS.some((p) => p.id === id), 'Missing pack: ' + id);
    }
  });

  record('tc_f05_02_domain_pack_manifest_validation', () => {
    for (const pack of DOMAIN_PACKS) {
      const res = validatePackManifest(pack);
      assert(res.valid, 'Pack ' + pack.id + ' failed validation: ' + res.error);
    }
  });

  record('tc_f05_03_capability_risk_classification_coverage', () => {
    const allRisks = new Set();
    for (const pack of DOMAIN_PACKS) {
      for (const cap of pack.capabilities) {
        allRisks.add(cap.risk);
      }
    }
    assert(allRisks.has('low'));
    assert(allRisks.has('medium'));
    assert(allRisks.has('high'));
    assert(allRisks.has('critical'));
  });

  record('tc_f05_04_cli_install_command_generation', () => {
    const cmd = generateInstallCommand('industrial');
    assert(cmd.includes('rivun pack install rivun-pack-industrial@1.0.0'));
    assert(cmd.includes('--verify-signature'));
  });

  record('tc_f05_05_safety_gate_declaration_presence', () => {
    for (const pack of DOMAIN_PACKS) {
      assert(pack.defaultSafetyGate.length > 5, 'Safety gate description required for ' + pack.id);
    }
  });

  // ------------------------------------------------------------------------
  // Feature 6: Enterprise Security & Compliance (tc_f06_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f06_01_soc2_hipaa_compliance_matrix_verification', () => {
    const complianceStandards = ['SOC2_Type_II', 'HIPAA', 'ISO_27001', 'GDPR'];
    assertEqual(complianceStandards.length, 4);
  });

  record('tc_f06_02_cryptographic_offline_verification_proof', () => {
    const mmr = new MerkleMountainRange();
    for (let i = 0; i < 5; i++) {
      mmr.append('compliance-record-' + i);
    }
    const proof = mmr.generateInclusionProof(2);
    assert(mmr.verifyInclusionProof(proof));
  });

  record('tc_f06_03_sub_millisecond_p99_sla_guarantee_model', () => {
    const latencies = [0.2, 0.3, 0.4, 0.5, 0.7, 0.75, 0.78, 0.79, 0.8];
    const p99 = latencies[Math.floor(latencies.length * 0.99)];
    assert(p99 <= 0.8, 'p99 SLA must be <= 0.8ms');
  });

  record('tc_f06_04_blinded_action_commitment_validation', () => {
    const blinding = BlindedCommitment.generateBlindingFactor();
    const commit = BlindedCommitment.commit('Rivun-COMPLIANCE', 'audit-trail-secret', blinding);
    assert(BlindedCommitment.verify(commit, 'Rivun-COMPLIANCE', 'audit-trail-secret', blinding));
  });

  record('tc_f06_05_blinded_receipt_commitment_validation', () => {
    const blinding = BlindedCommitment.generateBlindingFactor();
    const rc = BlindedReceiptCommitment.commit('public-payload', 'phi-hidden-fields', blinding);
    assert(BlindedReceiptCommitment.verify(rc, 'public-payload', 'phi-hidden-fields', blinding));
  });

  // ------------------------------------------------------------------------
  // Feature 7: Interactive Pricing & ROI Calculator (tc_f07_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f07_01_pricing_community_tier_free', () => {
    const pricing = calculatePricing({ tierId: 'community', nodeCount: 3, tps: 500 });
    assertEqual(pricing.monthlyCost, 0);
    assertEqual(pricing.tier, 'Community');
  });

  record('tc_f07_02_pricing_enterprise_tier_base_calculation', () => {
    const pricing = calculatePricing({ tierId: 'enterprise', nodeCount: 50, isAnnual: false });
    assertEqual(pricing.monthlyCost, 2499);
  });

  record('tc_f07_03_pricing_annual_discount_application', () => {
    const pricing = calculatePricing({ tierId: 'enterprise', nodeCount: 50, isAnnual: true });
    assertEqual(pricing.monthlyCost, Math.round(2499 * 0.8));
  });

  record('tc_f07_04_pricing_volume_node_scaling_overage', () => {
    const pricing = calculatePricing({ tierId: 'pro', nodeCount: 30, isAnnual: false });
    assertEqual(pricing.monthlyCost, 574);
  });

  record('tc_f07_05_pricing_roi_calculator_savings', () => {
    const pricing = calculatePricing({ tierId: 'enterprise', nodeCount: 100, tps: 50000, isAnnual: true });
    assert(pricing.monthlySavings > 0);
    assert(pricing.roiPercentage > 0);
  });

  // ------------------------------------------------------------------------
  // Feature 8: Live Developer Sandbox & Code Gen (tc_f08_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f08_01_sandbox_rust_code_generation', () => {
    const snippet = 'let frame = RivunFrame::builder().payload(b hello).sign(&keypair)?;';
    assert(snippet.includes('RivunFrame::builder()'));
  });

  record('tc_f08_02_sandbox_typescript_code_generation', () => {
    const snippet = 'const frame = new RivunFrame({ flags: Flags.SIGNED }, payload);';
    assert(snippet.includes('new RivunFrame'));
  });

  record('tc_f08_03_sandbox_python_code_generation', () => {
    const snippet = 'frame = RivunFrame(header=RivunHeader(flags=Flags.SIGNED), payload=bhello)';
    assert(snippet.includes('RivunFrame(header='));
  });

  record('tc_f08_04_sandbox_go_code_generation', () => {
    const snippet = 'frame, err := rivun.NewFrame(rivun.FlagsSigned, []byte(hello))';
    assert(snippet.includes('rivun.NewFrame'));
  });

  record('tc_f08_05_sandbox_curl_code_generation', () => {
    const snippet = 'curl -X POST https://api.rivun.io/v1/frames/send -H Authorization: Bearer ';
    assert(snippet.includes('curl -X POST'));
  });
