import { blake3, blake3Hex } from './blake3.mjs';
import { PROVENANCE_CHAIN_DOMAIN } from './crypto.mjs';

export const Stages = [
  'Intent',
  'Negotiation',
  'Policy',
  'Consensus',
  'Driver',
  'PoA',
  'Receipt',
];

export class ProvenanceChainBuilder {
  constructor() {
    this.stages = [];
  }

  addStage(stageName, stageData) {
    const dataBuf = Buffer.isBuffer(stageData)
      ? stageData
      : Buffer.from(typeof stageData === 'object' ? JSON.stringify(stageData) : String(stageData), 'utf8');
    const dataHash = blake3Hex(dataBuf);

    let stageHash;
    let prevHash = null;

    if (this.stages.length === 0) {
      stageHash = dataHash;
    } else {
      prevHash = this.stages[this.stages.length - 1].stageHash;
      stageHash = blake3Hex(Buffer.from(prevHash + ':' + dataHash, 'utf8'));
    }

    const stage = {
      index: this.stages.length,
      stageName,
      previousHash: prevHash,
      dataHash,
      stageHash,
    };
    this.stages.push(stage);
    return stage;
  }

  getRootHash() {
    if (this.stages.length === 0) {
      return Buffer.alloc(32, 0);
    }
    const lastHash = this.stages[this.stages.length - 1].stageHash;
    return blake3(Buffer.from(lastHash, 'utf8'));
  }

  seal(keypair) {
    const rootHash = this.getRootHash();
    const signature = keypair.signDomainMessage(PROVENANCE_CHAIN_DOMAIN, rootHash);
    return {
      nodeId: keypair.nodeId,
      stageCount: this.stages.length,
      stages: this.stages,
      rootHash: rootHash.toString('hex'),
      signature: signature.toString('hex'),
    };
  }

  static verify(chainDigest, publicKey) {
    if (!chainDigest || !chainDigest.stages || chainDigest.stages.length === 0) {
      return { valid: false, failureReason: 'Empty chain' };
    }

    for (let i = 0; i < chainDigest.stages.length; i++) {
      const stage = chainDigest.stages[i];
      if (i === 0) {
        if (stage.previousHash !== null) {
          return { valid: false, failureReason: 'Initial stage must have null previousHash' };
        }
        if (stage.stageHash !== stage.dataHash) {
          return { valid: false, failureReason: 'Initial stage hash mismatch' };
        }
      } else {
        const prev = chainDigest.stages[i - 1];
        if (stage.previousHash !== prev.stageHash) {
          return {
            valid: false,
            failureReason: 'Causal break at stage ' + stage.stageName + ' (expected ' + prev.stageHash + ', got ' + stage.previousHash + ')',
          };
        }
        const expected = blake3Hex(Buffer.from(stage.previousHash + ':' + stage.dataHash, 'utf8'));
        if (stage.stageHash !== expected) {
          return {
            valid: false,
            failureReason: 'Hash computation mismatch at stage ' + stage.stageName,
          };
        }
      }
    }

    const lastHash = chainDigest.stages[chainDigest.stages.length - 1].stageHash;
    const computedRoot = blake3(Buffer.from(lastHash, 'utf8'));
    if (computedRoot.toString('hex') !== chainDigest.rootHash) {
      return { valid: false, failureReason: 'Root hash mismatch' };
    }

    if (publicKey) {
      const sigBuf = Buffer.from(chainDigest.signature, 'hex');
      const validSig = publicKey.verifyDomainMessage(PROVENANCE_CHAIN_DOMAIN, computedRoot, sigBuf);
      if (!validSig) {
        return { valid: false, failureReason: 'Root signature invalid' };
      }
    }

    return { valid: true, rootHash: chainDigest.rootHash };
  }
}
