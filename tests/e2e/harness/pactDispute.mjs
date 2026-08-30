import { blake3, blake3Hex } from './blake3.mjs';
import { domainMessage } from './crypto.mjs';

export const PACT_DOMAIN = Buffer.from('ZAP-PACT-v1', 'utf8');
export const PACT_REVOCATION_DOMAIN = Buffer.from('ZAP-PACT-REVOCATION-v1', 'utf8');

export function canonicalizeJson(obj) {
  if (obj === null || typeof obj !== 'object') {
    return JSON.stringify(obj);
  }
  if (Array.isArray(obj)) {
    return '[' + obj.map(canonicalizeJson).join(',') + ']';
  }
  const keys = Object.keys(obj).sort();
  const pairs = keys.map((k) => JSON.stringify(k) + ':' + canonicalizeJson(obj[k]));
  return '{' + pairs.join(',') + '}';
}

export const PactState = {
  Proposed: 'Proposed',
  Locked: 'Locked',
  Settled: 'Settled',
  Disputed: 'Disputed',
  Slashed: 'Slashed',
};

export const RulingOutcome = {
  ReleaseToRecipient: 'ReleaseToRecipient',
  SlashRefundToSender: 'SlashRefundToSender',
  SplitEqual: 'SplitEqual',
};

export class EscrowPact {
  constructor({
    pactId,
    senderNode,
    recipientNode,
    escrowAmount,
    terms,
    arbitrators = [],
    arbitrationThreshold = 2,
    createdAtMicros = BigInt(Date.now()) * 1000n,
    expiresAtMicros = BigInt(Date.now() + 3600_000) * 1000n,
  }) {
    this.pactId = pactId;
    this.senderNode = senderNode;
    this.recipientNode = recipientNode;
    this.escrowAmount = escrowAmount;
    this.terms = terms;
    this.arbitrators = arbitrators; // array of arbitrator nodeIds
    this.arbitrationThreshold = arbitrationThreshold;
    this.createdAtMicros = BigInt(createdAtMicros);
    this.expiresAtMicros = BigInt(expiresAtMicros);
    this.state = PactState.Proposed;
    this.senderSignature = null;
    this.recipientSignature = null;
    this.disputeEvidence = [];
    this.arbitrationVotes = []; // { arbitratorNode, ruling, signature }
    this.finalRuling = null;
  }

  get signatures() {
    const m = new Map();
    if (this.senderSignature) m.set(this.senderNode, this.senderSignature);
    if (this.recipientSignature) m.set(this.recipientNode, this.recipientSignature);
    return m;
  }

  get ruling() {
    return this.finalRuling;
  }

  canonicalPayload() {
    const canon = {
      pact_id: this.pactId,
      sender_node: this.senderNode,
      recipient_node: this.recipientNode,
      escrow_amount: this.escrowAmount,
      terms: this.terms,
      arbitrators: this.arbitrators,
      arbitration_threshold: this.arbitrationThreshold,
      created_at_micros: this.createdAtMicros.toString(),
      expires_at_micros: this.expiresAtMicros.toString(),
    };
    return Buffer.from(canonicalizeJson(canon), 'utf8');
  }

  digest() {
    return blake3(this.canonicalPayload());
  }

  sign(keypair) {
    if (this.state !== PactState.Proposed) {
      throw new Error('Cannot sign pact in state ' + this.state);
    }
    const msg = domainMessage(PACT_DOMAIN, this.digest());
    const sig = keypair.sign(msg);

    if (keypair.nodeId === this.senderNode) {
      this.senderSignature = sig;
    } else if (keypair.nodeId === this.recipientNode) {
      this.recipientSignature = sig;
    } else {
      throw new Error('Node ' + keypair.nodeId + ' is neither sender nor recipient');
    }

    if (this.senderSignature && this.recipientSignature) {
      this.state = PactState.Locked;
    }
    return sig;
  }

  settle(senderKeypair) {
    if (this.state !== PactState.Locked) {
      throw new Error('Cannot settle pact in state ' + this.state);
    }
    if (senderKeypair.nodeId !== this.senderNode) {
      throw new Error('Only sender can settle release');
    }
    this.state = PactState.Settled;
    this.finalRuling = RulingOutcome.ReleaseToRecipient;
  }

  raiseDispute(partyKeypair, reason, evidence) {
    if (this.state !== PactState.Locked) {
      throw new Error('Cannot dispute pact in state ' + this.state);
    }
    if (partyKeypair.nodeId !== this.senderNode && partyKeypair.nodeId !== this.recipientNode) {
      throw new Error('Only pact participants can raise a dispute');
    }
    this.state = PactState.Disputed;
    this.disputeEvidence.push({
      party: partyKeypair.nodeId,
      reason,
      evidence,
      timestamp: Date.now(),
    });
  }

  castArbitrationVote(arbitratorKeypair, rulingOutcome) {
    if (this.state !== PactState.Disputed) {
      throw new Error('Cannot arbitrate pact in state ' + this.state);
    }
    if (!this.arbitrators.includes(arbitratorKeypair.nodeId)) {
      throw new Error('Node ' + arbitratorKeypair.nodeId + ' is not an authorized arbitrator');
    }

    const voteMsg = Buffer.concat([
      this.digest(),
      Buffer.from(rulingOutcome, 'utf8'),
    ]);
    const sig = arbitratorKeypair.sign(domainMessage(PACT_DOMAIN, voteMsg));

    this.arbitrationVotes.push({
      arbitratorNode: arbitratorKeypair.nodeId,
      ruling: rulingOutcome,
      signature: sig,
    });

    // Check if arbitration threshold is reached
    const counts = {};
    for (const vote of this.arbitrationVotes) {
      counts[vote.ruling] = (counts[vote.ruling] || 0) + 1;
      if (counts[vote.ruling] >= this.arbitrationThreshold) {
        this.finalRuling = vote.ruling;
        this.state = vote.ruling === RulingOutcome.ReleaseToRecipient ? PactState.Settled : PactState.Slashed;
        break;
      }
    }
  }
}
