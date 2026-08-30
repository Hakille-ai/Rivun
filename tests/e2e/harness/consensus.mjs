import { blake3, blake3Hex } from './blake3.mjs';
import { domainMessage } from './crypto.mjs';

export const BFT_DOMAIN = Buffer.from('Rivun-BFT-CONSENSUS-v1', 'utf8');

export function calculateQuorumThreshold(n) {
  if (n <= 0) return 0;
  return Math.floor((2 * n) / 3) + 1;
}

export class BftConsensusEngine {
  constructor({ epoch = 1, validators = [], threshold = null }) {
    this.epoch = epoch;
    this.round = 0;
    this.validators = validators; // array of { keypair/publicKey, nodeId }
    this.threshold = threshold !== null ? threshold : calculateQuorumThreshold(validators.length);
    this.proposals = new Map(); // key: (epoch:round) -> proposal
    this.prevotes = new Map();  // key: (epoch:round:nodeId) -> vote
    this.precommits = new Map(); // key: (epoch:round:nodeId) -> vote
    this.slashedNodes = new Set();
    this.equivocations = [];
    this.committedHeight = 0;
  }

  propose(leaderKeypair, height, proposalHash) {
    if (this.slashedNodes.has(leaderKeypair.nodeId)) {
      throw new Error('Slashed validator cannot propose');
    }
    const key = this.epoch + ':' + this.round + ':' + height;
    const msg = domainMessage(
      BFT_DOMAIN,
      Buffer.from('PROPOSE:' + key + ':' + proposalHash, 'utf8')
    );
    const signature = leaderKeypair.sign(msg);
    const proposal = {
      epoch: this.epoch,
      round: this.round,
      height,
      proposerNode: leaderKeypair.nodeId,
      proposalHash,
      signature,
    };

    if (this.proposals.has(key)) {
      const existing = this.proposals.get(key);
      if (existing.proposerNode === leaderKeypair.nodeId && existing.proposalHash !== proposalHash) {
        // Equivocation!
        this.slashedNodes.add(leaderKeypair.nodeId);
        this.equivocations.push({
          type: 'proposal_equivocation',
          nodeId: leaderKeypair.nodeId,
          first: existing,
          second: proposal,
        });
        throw new Error('Equivocation detected: leader slashed!');
      }
    }
    this.proposals.set(key, proposal);
    return proposal;
  }

  castPrevote(validatorKeypair, height, proposalHash) {
    if (this.slashedNodes.has(validatorKeypair.nodeId)) {
      throw new Error('Slashed validator cannot vote');
    }
    const key = this.epoch + ':' + this.round + ':' + height + ':' + validatorKeypair.nodeId;
    const msg = domainMessage(
      BFT_DOMAIN,
      Buffer.from('PREVOTE:' + this.epoch + ':' + this.round + ':' + height + ':' + proposalHash, 'utf8')
    );
    const signature = validatorKeypair.sign(msg);
    const vote = {
      step: 'PREVOTE',
      epoch: this.epoch,
      round: this.round,
      height,
      validatorNode: validatorKeypair.nodeId,
      proposalHash,
      signature,
    };

    if (this.prevotes.has(key)) {
      const existing = this.prevotes.get(key);
      if (existing.proposalHash !== proposalHash) {
        this.slashedNodes.add(validatorKeypair.nodeId);
        this.equivocations.push({
          type: 'prevote_equivocation',
          nodeId: validatorKeypair.nodeId,
          first: existing,
          second: vote,
        });
        throw new Error('Equivocation detected: validator slashed!');
      }
    }
    this.prevotes.set(key, vote);
    return vote;
  }

  castPrecommit(validatorKeypair, height, proposalHash) {
    if (this.slashedNodes.has(validatorKeypair.nodeId)) {
      throw new Error('Slashed validator cannot vote');
    }
    const key = this.epoch + ':' + this.round + ':' + height + ':' + validatorKeypair.nodeId;
    const msg = domainMessage(
      BFT_DOMAIN,
      Buffer.from('PRECOMMIT:' + this.epoch + ':' + this.round + ':' + height + ':' + proposalHash, 'utf8')
    );
    const signature = validatorKeypair.sign(msg);
    const vote = {
      step: 'PRECOMMIT',
      epoch: this.epoch,
      round: this.round,
      height,
      validatorNode: validatorKeypair.nodeId,
      proposalHash,
      signature,
    };

    if (this.precommits.has(key)) {
      const existing = this.precommits.get(key);
      if (existing.proposalHash !== proposalHash) {
        this.slashedNodes.add(validatorKeypair.nodeId);
        this.equivocations.push({
          type: 'precommit_equivocation',
          nodeId: validatorKeypair.nodeId,
          first: existing,
          second: vote,
        });
        throw new Error('Equivocation detected: validator slashed!');
      }
    }
    this.precommits.set(key, vote);
    return vote;
  }

  checkPolka(height, proposalHash) {
    let count = 0;
    for (const [key, vote] of this.prevotes.entries()) {
      if (
        vote.epoch === this.epoch &&
        vote.round === this.round &&
        vote.height === height &&
        vote.proposalHash === proposalHash &&
        !this.slashedNodes.has(vote.validatorNode)
      ) {
        count++;
      }
    }
    return count >= this.threshold;
  }

  createCommitCertificate(height, proposalHash, publicKeys) {
    const validPrecommits = [];
    const bitmaskBytes = Math.ceil(this.validators.length / 8);
    const bitmask = Buffer.alloc(bitmaskBytes, 0);

    for (let i = 0; i < this.validators.length; i++) {
      const val = this.validators[i];
      const key = this.epoch + ':' + this.round + ':' + height + ':' + val.nodeId;
      const vote = this.precommits.get(key);
      if (
        vote &&
        vote.proposalHash === proposalHash &&
        !this.slashedNodes.has(val.nodeId)
      ) {
        // Verify signature
        const pk = publicKeys.find((p) => p.nodeId === val.nodeId);
        if (pk) {
          const msg = domainMessage(
            BFT_DOMAIN,
            Buffer.from('PRECOMMIT:' + this.epoch + ':' + this.round + ':' + height + ':' + proposalHash, 'utf8')
          );
          if (pk.verify(msg, vote.signature)) {
            bitmask[Math.floor(i / 8)] |= (1 << (i % 8));
            validPrecommits.push(vote);
          }
        }
      }
    }

    if (validPrecommits.length < this.threshold) {
      throw new Error('Quorum threshold not met: required ' + this.threshold + ', got ' + validPrecommits.length);
    }

    this.committedHeight = height;
    return {
      epoch: this.epoch,
      round: this.round,
      height,
      proposalHash,
      threshold: this.threshold,
      validatorCount: this.validators.length,
      attestationCount: validPrecommits.length,
      bitmask: bitmask.toString('hex'),
      signatures: validPrecommits.map((v) => ({
        validatorNode: v.validatorNode,
        signature: v.signature.toString('hex'),
      })),
    };
  }
}
