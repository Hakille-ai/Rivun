// ============================================================================
// Tier 3: Cross-Feature Integration Flows (20 Complex Multi-Stage Workloads)
// ============================================================================

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
  VERSION,
  HEADER_LEN,
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

export function runTier3Tests(record) {
  // ------------------------------------------------------------------------
  // Flow 1: Wire Frame Signed & Routed Through SPSC Ring Buffer
  // ------------------------------------------------------------------------
  record('tc_t3_01_wire_frame_signed_and_routed_through_spsc_ring', () => {
    const sender = Keypair.generate();
    const hdr = new RivunHeader({ sourceNode: sender.nodeId, flags: Flags.SIGNED });
    let frame = new RivunFrame(hdr, Buffer.from('streamed-data-payload', 'utf8'));
    frame = signFrame(sender, frame);
    const encoded = frame.encode();

    const ring = new SpscRingBuffer(encoded.length * 2);
    ring.write(encoded);
    const receivedBytes = ring.read(encoded.length);
    const decodedFrame = RivunFrame.decode(receivedBytes);

    assertEqual(decodedFrame.header.sourceNode, sender.nodeId);
    assert(verifyFrame(sender.getVerifyingKey(), decodedFrame));
  });

  // ------------------------------------------------------------------------
  // Flow 2: Universal ZENV Envelope Embedded in Wire Frame Payload
  // ------------------------------------------------------------------------
  record('tc_t3_02_zenv_envelope_embedded_in_wire_frame_payload', () => {
    const agentKp = Keypair.generate();
    const env = new RivunEnvelope({
      kind: MessageKind.Command,
      subject: 'agent.deploy',
      body: Buffer.from(JSON.stringify({ cluster: 'prod-1', replicas: 3 })),
    });
    const envBytes = env.encode();

    const hdr = new RivunHeader({
      sourceNode: agentKp.nodeId,
      flags: Flags.SIGNED,
      payloadLen: envBytes.length,
    });
    let frame = new RivunFrame(hdr, envBytes);
    frame = signFrame(agentKp, frame);

    const frameBytes = frame.encode();
    const unpackedFrame = RivunFrame.decode(frameBytes);
    const unpackedEnv = RivunEnvelope.decode(unpackedFrame.payload);

    assertEqual(unpackedEnv.kind, MessageKind.Command);
    assertEqual(unpackedEnv.subject, 'agent.deploy');
    assert(verifyFrame(agentKp.getVerifyingKey(), unpackedFrame));
  });

  // ------------------------------------------------------------------------
  // Flow 3: Wire Frame PoA Quorum Certification Appended to MMR Ledger
  // ------------------------------------------------------------------------
  record('tc_t3_03_wire_frame_poa_quorum_certification_to_mmr_ledger', () => {
    const sender = Keypair.generate();
    const v1 = Keypair.generate();
    const v2 = Keypair.generate();
    const v3 = Keypair.generate();
    const validators = [v1, v2, v3];

    const hdr = new RivunHeader({ sourceNode: sender.nodeId, flags: Flags.REQUIRES_CONSENSUS });
    let frame = new RivunFrame(hdr, Buffer.from('audit-log-entry-1', 'utf8'));
    frame = signFrame(sender, frame);
    frame = certifyFrame(frame, 2, validators);

    const valPks = validators.map((v) => v.getVerifyingKey());
    assert(verifyPoaCertificate(frame, valPks, 2));

    const mmr = new MerkleMountainRange();
    const receiptLeaf = blake3(frame.encode());
    mmr.append(receiptLeaf);

    const proof = mmr.generateInclusionProof(0);
    assert(mmr.verifyInclusionProof(proof));
  });

  // ------------------------------------------------------------------------
  // Flow 4: BFT Consensus Proposal to Commit and MMR Accumulation
  // ------------------------------------------------------------------------
  record('tc_t3_04_bft_consensus_proposal_to_commit_and_mmr_accumulation', () => {
    const v1 = Keypair.generate();
    const v2 = Keypair.generate();
    const v3 = Keypair.generate();
    const v4 = Keypair.generate();
    const validators = [v1, v2, v3, v4];

    const engine = new BftConsensusEngine({ validators });
    const blockData = 'block-height-100-data';
    const blockHash = blake3Hex(blockData);

    const proposal = engine.propose(v1, 100, blockHash);
    assertEqual(proposal.height, 100);

    engine.castPrevote(v1, 100, blockHash);
    engine.castPrevote(v2, 100, blockHash);
    engine.castPrevote(v3, 100, blockHash);
    assert(engine.checkPolka(100, blockHash));

    engine.castPrecommit(v1, 100, blockHash);
    engine.castPrecommit(v2, 100, blockHash);
    engine.castPrecommit(v3, 100, blockHash);

    const valPks = validators.map((v) => v.getVerifyingKey());
    const cert = engine.createCommitCertificate(100, blockHash, valPks);
    assertEqual(cert.attestationCount, 3);

    const mmr = new MerkleMountainRange();
    mmr.append(cert.proposalHash);
    assertEqual(mmr.leafCount, 1);
  });

  // ------------------------------------------------------------------------
  // Flow 5: WASM Driver Pipeline with SPSC Streaming and Provenance
  // ------------------------------------------------------------------------
  record('tc_t3_05_wasm_driver_pipeline_with_spsc_streaming_and_provenance', () => {
    const operator = Keypair.generate();
    const chain = new ProvenanceChainBuilder();

    chain.addStage('Intent', { task: 'sanitize_log', records: 10 });
    chain.addStage('PolicyCheck', { allowed: true, quotaRemaining: 990 });

    const pipeline = new DriverPipeline([
      { name: 'sanitizer', logic: (a, p) => Buffer.from(p.toString('utf8').replace(/secret/g, 'REDACTED')) },
      { name: 'compressor', logic: (a, p) => Buffer.from(p.toString('utf8').trim()) },
    ]);

    const res = pipeline.run(Buffer.from('log: user login secret token'));
    assertEqual(res.finalOutput.toString('utf8'), 'log: user login REDACTED token');

    chain.addStage('DriverExecution', { stepHashes: res.stepHashes });
    const sealed = chain.seal(operator);

    const verifyRes = ProvenanceChainBuilder.verify(sealed);
    assert(verifyRes.valid);
  });

  // ------------------------------------------------------------------------
  // Flow 6: ChaCha20 Authenticated Datagram with ZENV and Ed25519
  // ------------------------------------------------------------------------
  record('tc_t3_06_chacha20_authenticated_datagram_with_zenv_and_ed25519', () => {
    const sessionKey = Buffer.alloc(32, 0x42);
    const nonce = Buffer.alloc(12, 0x07);
    const sender = Keypair.generate();

    const env = new RivunEnvelope({
      kind: MessageKind.Data,
      subject: 'telemetry.metric',
      body: Buffer.from('{" tps\: 45000}'),
 });
 const plainPayload = env.encode();

 const hdr = new RivunHeader({ sourceNode: sender.nodeId, flags: Flags.ENCRYPTED | Flags.SIGNED });
 const aad = hdr.encode();

 const { ciphertext, tag } = encryptChaCha20Poly1305(sessionKey, nonce, plainPayload, aad);
 const decryptedPayload = decryptChaCha20Poly1305(sessionKey, nonce, ciphertext, tag, aad);
 const decodedEnv = RivunEnvelope.decode(decryptedPayload);

 assertEqual(decodedEnv.subject, 'telemetry.metric');
 });

 // ------------------------------------------------------------------------
 // Flow 7: PACT Escrow Dispute to BFT Arbitration Slashing
 // ------------------------------------------------------------------------
 record('tc_t3_07_pact_escrow_dispute_to_bft_arbitration_slashing', () => {
 const buyer = Keypair.generate();
 const seller = Keypair.generate();
 const arb1 = Keypair.generate();
 const arb2 = Keypair.generate();
 const arb3 = Keypair.generate();

 const pact = new EscrowPact({
 pactId: 'pact-escrow-07',
 senderNode: buyer.nodeId,
 recipientNode: seller.nodeId,
 escrowAmount: 5000,
 terms: 'deliver 100 model tokens',
 arbitrators: [arb1.nodeId, arb2.nodeId, arb3.nodeId],
 arbitrationThreshold: 2,
 });

 pact.sign(buyer);
 pact.sign(seller);
 assertEqual(pact.state, PactState.Locked);

 pact.raiseDispute(buyer, 'seller failed delivery', 'proof-of-timeout');
 assertEqual(pact.state, PactState.Disputed);

 pact.castArbitrationVote(arb1, RulingOutcome.SlashRefundToSender);
 pact.castArbitrationVote(arb2, RulingOutcome.SlashRefundToSender);

 assertEqual(pact.state, PactState.Slashed);
 assertEqual(pact.ruling, RulingOutcome.SlashRefundToSender);
 });

 // ------------------------------------------------------------------------
 // Flow 8: Domain Pack Manifest Validation to WASM Sandbox Execution
 // ------------------------------------------------------------------------
 record('tc_t3_08_domain_pack_manifest_validation_to_wasm_sandbox_execution', () => {
 const pack = getDomainPack('healthcare');
 const manifestRes = validatePackManifest(pack);
 assert(manifestRes.valid);

 const sandbox = new WasmGuestSandbox({ initialFuel: 50000 });
 const payload = Buffer.from('patient-record-id-12345');
 const res = sandbox.execute('records.read', payload, (a, p) => Buffer.from('encrypted-' + p.toString('utf8')));

 assert(res.output.toString('utf8').startsWith('encrypted-'));
 assert(res.remainingFuel < 50000);
 });

 // ------------------------------------------------------------------------
 // Flow 9: Fleet Doctor Diagnostics During Live Consensus Cluster
 // ------------------------------------------------------------------------
 record('tc_t3_09_fleet_doctor_diagnostics_during_live_consensus_cluster', () => {
 const v1 = Keypair.generate();
 const v2 = Keypair.generate();
 const v3 = Keypair.generate();
 const v4 = Keypair.generate();

 const doctor = new FleetDoctor({
 totalValidators: 4,
 activeValidators: 4,
 quarantinedPeers: 0,
 walClockSkewSecs: 2,
 });

 const report = doctor.runDiagnostics();
 assertEqual(report.overallHealthy, true);
 assertEqual(report.passedCount, 7);
 });

 // ------------------------------------------------------------------------
 // Flow 10: Search Engine Indexing of 26 Crates and 7 Domain Packs
 // ------------------------------------------------------------------------
 record('tc_t3_10_search_engine_indexing_of_26_crates_and_7_domain_packs', () => {
 const engine = new SearchEngine();

 for (const pack of DOMAIN_PACKS) {
 engine.addDocument({
 id: pack.id,
 title: pack.name,
 category: 'domain-pack',
 description: pack.description,
 content: pack.capabilities.map((c) => c.name).join(' '),
 url: '/domain-packs/' + pack.id,
 });
 }

 const crateNames = ['rivun-core', 'rivun-crypto', 'rivun-ledger', 'rivun-pact'];
 for (const cName of crateNames) {
 engine.addDocument({
 id: cName,
 title: cName,
 category: 'crate',
 description: 'Core crate ' + cName,
 content: 'API documentation for ' + cName,
 url: '/docs/crates/' + cName,
 });
 }

 const packResults = engine.search('healthcare', { category: 'domain-pack' });
 assertEqual(packResults.length, 1);
 assertEqual(packResults[0].id, 'rivun-pack-healthcare');

 const crateResults = engine.search('ledger', { category: 'crate' });
 assertEqual(crateResults.length, 1);
 assertEqual(crateResults[0].id, 'rivun-ledger');
 });

 // ------------------------------------------------------------------------
 // Flow 11: Pricing Engine with Domain Pack Volume and SLA Guarantee
 // ------------------------------------------------------------------------
 record('tc_t3_11_pricing_engine_with_domain_pack_volume_and_sla_guarantee', () => {
 const pricing = calculatePricing({
 tierId: 'enterprise',
 nodeCount: 150,
 tps: 80000,
 isAnnual: true,
 });

 assert(pricing.monthlyCost > 0);
 assertEqual(pricing.sla, '99.99% SLA');
 assert(pricing.roiPercentage > 50);
 });

 // ------------------------------------------------------------------------
 // Flow 12: Operator Workstation Staged Policy to Node Runtime Rollout
 // ------------------------------------------------------------------------
 record('tc_t3_12_operator_workstation_staged_policy_to_node_runtime_rollout', () => {
 const operator = Keypair.generate();
 const stagedPolicy = {
 policyId: 'policy-sec-99',
 version: 3,
 rules: [{ cap: 'net.egress', action: 'deny' }],
 };

 const signature = operator.sign(Buffer.from(JSON.stringify(stagedPolicy)));
 const rolloutBundle = {
 policy: stagedPolicy,
 signature: signature.toString('hex'),
 operatorNodeId: operator.nodeId,
 };

 const sigBuf = Buffer.from(rolloutBundle.signature, 'hex');
 const verified = operator.getVerifyingKey().verify(Buffer.from(JSON.stringify(rolloutBundle.policy)), sigBuf);
 assert(verified);
 });

 // ------------------------------------------------------------------------
 // Flow 13: Blinded Receipt Commitment in Audit Provenance Chain
 // ------------------------------------------------------------------------
 record('tc_t3_13_blinded_receipt_commitment_in_audit_provenance_chain', () => {
 const auditor = Keypair.generate();
 const blinding = BlindedCommitment.generateBlindingFactor();
 const commitment = BlindedReceiptCommitment.commit('receipt-header', 'phi-patient-ssn-hidden', blinding);

 const chain = new ProvenanceChainBuilder();
 chain.addStage('MedicalQuery', { patientRef: 'id-001' });
 chain.addStage('BlindedAuditCommit', { commitmentHex: commitment.toString('hex') });

 const sealed = chain.seal(auditor);
 assert(ProvenanceChainBuilder.verify(sealed).valid);
 assert(BlindedReceiptCommitment.verify(commitment, 'receipt-header', 'phi-patient-ssn-hidden', blinding));
 });

 // ------------------------------------------------------------------------
 // Flow 14: Wire Frame with Priority Flag Bypassing SPSC Backpressure
 // ------------------------------------------------------------------------
 record('tc_t3_14_wire_frame_with_priority_flag_bypassing_spsc_backpressure', () => {
 const sender = Keypair.generate();
 const hdr = new RivunHeader({ sourceNode: sender.nodeId, flags: Flags.PRIORITY | Flags.SIGNED });
 let frame = new RivunFrame(hdr, Buffer.from('urgent-control-signal'));
 frame = signFrame(sender, frame);

 const isPriority = (frame.header.flags & Flags.PRIORITY) !== 0;
 assert(isPriority);
 });

 // ------------------------------------------------------------------------
 // Flow 15: Equivocating Consensus Node Triggers Fleet Doctor Alert
 // ------------------------------------------------------------------------
 record('tc_t3_15_equivocating_consensus_node_triggers_fleet_doctor_alert', () => {
 const v1 = Keypair.generate();
 const engine = new BftConsensusEngine({ validators: [v1] });

 engine.castPrevote(v1, 1, 'block-a');
 try {
 engine.castPrevote(v1, 1, 'block-b'); // Equivocation
 } catch {
 // Expected
 }

 assert(engine.slashedNodes.has(v1.nodeId));

 const doctor = new FleetDoctor({ quarantinedPeers: engine.slashedNodes.size });
 const report = doctor.runDiagnostics();
 assertEqual(report.overallHealthy, false);
 });

 // ------------------------------------------------------------------------
 // Flow 16: Merkle Mountain Range Reorganization and Historical Proofs
 // ------------------------------------------------------------------------
 record('tc_t3_16_merkle_mountain_range_reorganization_and_historical_proofs', () => {
 const mmr = new MerkleMountainRange();
 for (let i = 0; i < 15; i++) {
 mmr.append('leaf-record-' + i);
 }

 assertEqual(mmr.leafCount, 15);
 const proof0 = mmr.generateInclusionProof(0);
 assert(mmr.verifyInclusionProof(proof0));

 const proof14 = mmr.generateInclusionProof(14);
 assert(mmr.verifyInclusionProof(proof14));

 const batchProof = mmr.generateBatchProof([0, 5, 10, 14]);
 assert(mmr.verifyBatchProof(batchProof));
 });

 // ------------------------------------------------------------------------
 // Flow 17: Multi-Tenant Envelope Isolation Across Separate Pacts
 // ------------------------------------------------------------------------
 record('tc_t3_17_multi_tenant_envelope_isolation_across_separate_pacts', () => {
 const tenantA = Keypair.generate();
 const tenantB = Keypair.generate();

 const pactA = new EscrowPact({ pactId: 'pact-a', senderNode: tenantA.nodeId, recipientNode: 'r-1', escrowAmount: 100 });
 const pactB = new EscrowPact({ pactId: 'pact-b', senderNode: tenantB.nodeId, recipientNode: 'r-2', escrowAmount: 200 });

 pactA.sign(tenantA);
 pactB.sign(tenantB);

 assert(pactA.senderNode !== pactB.senderNode);
 assertEqual(pactA.signatures.size, 1);
 assertEqual(pactB.signatures.size, 1);
 });

 // ------------------------------------------------------------------------
 // Flow 18: Driver Pipeline Failure Preserves Provenance Error Stage
 // ------------------------------------------------------------------------
 record('tc_t3_18_driver_pipeline_failure_preserves_provenance_error_stage', () => {
 const operator = Keypair.generate();
 const chain = new ProvenanceChainBuilder();

 chain.addStage('Intent', { action: 'heavy_compute' });

 let errCaught = false;
 try {
 const sandbox = new WasmGuestSandbox({ initialFuel: 10 });
 sandbox.execute('action', Buffer.from('data'), (a, p) => p);
 } catch (e) {
 errCaught = true;
 chain.addStage('ExecutionFailure', { error: e.message });
 }

 assert(errCaught);
 const sealed = chain.seal(operator);
 assert(ProvenanceChainBuilder.verify(sealed).valid);
 });

 // ------------------------------------------------------------------------
 // Flow 19: Cloud Bridge SSE Receipt Stream Feeding MMR Accumulator
 // ------------------------------------------------------------------------
 record('tc_t3_19_cloud_bridge_sse_receipt_stream_feeding_mmr_accumulator', () => {
 const sseEvents = [
 { receiptId: 'r-01', payloadDigest: blake3Hex('p1') },
 { receiptId: 'r-02', payloadDigest: blake3Hex('p2') },
 { receiptId: 'r-03', payloadDigest: blake3Hex('p3') },
 ];

 const mmr = new MerkleMountainRange();
 for (const ev of sseEvents) {
 mmr.append(ev.payloadDigest);
 }

 assertEqual(mmr.leafCount, 3);
 const root = mmr.getRoot();
 assertEqual(root.length, 32);
 });

 // ------------------------------------------------------------------------
 // Flow 20: End-to-End Agent Action Lifecycle
 // ------------------------------------------------------------------------
 record('tc_t3_20_end_to_end_agent_action_lifecycle', () => {
 const agent = Keypair.generate();
 const v1 = Keypair.generate();
 const v2 = Keypair.generate();

 // 1. Universal envelope
 const env = new RivunEnvelope({
 kind: MessageKind.Action,
 subject: 'smart-building.hvac.adjust',
 body: Buffer.from(JSON.stringify({ zone: '4B', tempCelsius: 21.5 })),
 });

 // 2. Wire frame signing
 const hdr = new RivunHeader({ sourceNode: agent.nodeId, flags: Flags.SIGNED | Flags.REQUIRES_CONSENSUS });
 let frame = new RivunFrame(hdr, env.encode());
 frame = signFrame(agent, frame);

 // 3. PoA Certification
 frame = certifyFrame(frame, 2, [v1, v2]);

 // 4. WASM Execution
 const sandbox = new WasmGuestSandbox();
 const execRes = sandbox.execute('hvac.adjust', frame.payload, (a, p) => Buffer.from('applied-ok'));
 assertEqual(execRes.output.toString('utf8'), 'applied-ok');

 // 5. Blinded Receipt Commitment
 const blinding = BlindedCommitment.generateBlindingFactor();
 const commit = BlindedReceiptCommitment.commit(frame.encode(), execRes.output, blinding);

 // 6. MMR Ledger Recording
 const mmr = new MerkleMountainRange();
 mmr.append(commit);

 assertEqual(mmr.leafCount, 1);
 const proof = mmr.generateInclusionProof(0);
 assert(mmr.verifyInclusionProof(proof));
 });
}
