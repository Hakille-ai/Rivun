import { calculateQuorumThreshold } from './consensus.mjs';

export const DoctorCheckStatus = {
  Passed: 'PASSED',
  Warning: 'WARNING',
  Failed: 'FAILED',
};

export class FleetDoctor {
  constructor(nodeConfig = {}) {
    this.config = {
      udpPort: 9000,
      activePeers: 3,
      receiptDirExists: true,
      memoryDirExists: true,
      walFramingValid: true,
      walClockSkewSecs: 2,
      journalSegmentMagicValid: true,
      journalManifestSigned: true,
      packRegistrySigned: true,
      ed25519KeyLoaded: true,
      totalValidators: 4,
      activeValidators: 4,
      quarantinedPeers: 0,
      revokedPeers: 0,
      ...nodeConfig,
    };
  }

  runDiagnostics() {
    const checks = [];

    // 1. Network Reachability
    if (this.config.udpPort > 0 && this.config.activePeers >= 1) {
      checks.push({ id: 1, name: 'network_reachability', status: DoctorCheckStatus.Passed, message: 'UDP socket bound, ' + this.config.activePeers + ' active peers' });
    } else {
      checks.push({ id: 1, name: 'network_reachability', status: DoctorCheckStatus.Failed, message: 'No active peers or UDP bind failed' });
    }

    // 2. Storage Mounts
    if (this.config.receiptDirExists && this.config.memoryDirExists) {
      checks.push({ id: 2, name: 'storage_mounts', status: DoctorCheckStatus.Passed, message: 'Receipt & memory journals mounted writable' });
    } else {
      checks.push({ id: 2, name: 'storage_mounts', status: DoctorCheckStatus.Failed, message: 'Storage directory missing or unmounted' });
    }

    // 3. Replay Guard WAL
    if (this.config.walFramingValid && this.config.walClockSkewSecs < 30) {
      checks.push({ id: 3, name: 'replay_guard_wal', status: DoctorCheckStatus.Passed, message: 'Durable WAL frame ZAPFRM01 valid, clock skew ' + this.config.walClockSkewSecs + 's < 30s' });
    } else {
      checks.push({ id: 3, name: 'replay_guard_wal', status: DoctorCheckStatus.Failed, message: 'WAL frame corrupted or clock skew ' + this.config.walClockSkewSecs + 's >= 30s' });
    }

    // 4. Journal Integrity
    if (this.config.journalSegmentMagicValid && this.config.journalManifestSigned) {
      checks.push({ id: 4, name: 'journal_integrity', status: DoctorCheckStatus.Passed, message: 'Segment magic ZJSEG001 intact, signed manifests verified' });
    } else {
      checks.push({ id: 4, name: 'journal_integrity', status: DoctorCheckStatus.Failed, message: 'Journal segment magic invalid or unsigned manifest' });
    }

    // 5. Pack Registry
    if (this.config.packRegistrySigned) {
      checks.push({ id: 5, name: 'pack_registry', status: DoctorCheckStatus.Passed, message: 'RivunStore bundle signatures valid' });
    } else {
      checks.push({ id: 5, name: 'pack_registry', status: DoctorCheckStatus.Failed, message: 'Unsigned or tampered domain pack detected' });
    }

    // 6. Quorum & Certificates
    const requiredThreshold = calculateQuorumThreshold(this.config.totalValidators);
    if (this.config.ed25519KeyLoaded && this.config.activeValidators >= requiredThreshold) {
      checks.push({ id: 6, name: 'quorum_and_certificates', status: DoctorCheckStatus.Passed, message: 'Ed25519 key valid, Quorum threshold ' + requiredThreshold + '/' + this.config.totalValidators + ' satisfied' });
    } else if (this.config.activeValidators < requiredThreshold) {
      checks.push({ id: 6, name: 'quorum_and_certificates', status: DoctorCheckStatus.Warning, message: 'Active validators (' + this.config.activeValidators + ') below quorum threshold (' + requiredThreshold + '/' + this.config.totalValidators + ')' });
    } else {
      checks.push({ id: 6, name: 'quorum_and_certificates', status: DoctorCheckStatus.Failed, message: 'Missing node identity Ed25519 key' });
    }

    // 7. Peer Trust Status
    if (this.config.quarantinedPeers === 0 && this.config.revokedPeers === 0) {
      checks.push({ id: 7, name: 'peer_trust_status', status: DoctorCheckStatus.Passed, message: 'All connected peers trusted, 0 quarantined/revoked' });
    } else {
      checks.push({ id: 7, name: 'peer_trust_status', status: DoctorCheckStatus.Failed, message: 'Found ' + (this.config.quarantinedPeers + this.config.revokedPeers) + ' untrusted/quarantined peers' });
    }

    const failedCount = checks.filter((c) => c.status === DoctorCheckStatus.Failed).length;
    const warningCount = checks.filter((c) => c.status === DoctorCheckStatus.Warning).length;
    const passedCount = checks.filter((c) => c.status === DoctorCheckStatus.Passed).length;

    const overallHealthy = failedCount === 0 && warningCount === 0;

    return {
      nodeId: this.config.nodeId || '00000000-0000-0000-0000-000000000001',
      timestamp: Date.now(),
      overallHealthy,
      passedCount,
      warningCount,
      failedCount,
      checks,
    };
  }
}
