import { blake3, blake3Hex } from './blake3.mjs';

export function mmrParentHash(leftHash, rightHash) {
  const l = Buffer.isBuffer(leftHash) ? leftHash : Buffer.from(leftHash, 'hex');
  const r = Buffer.isBuffer(rightHash) ? rightHash : Buffer.from(rightHash, 'hex');
  return blake3(Buffer.concat([l, r]));
}

export function bagPeaks(peaks) {
  if (!peaks || peaks.length === 0) {
    return Buffer.alloc(32, 0);
  }
  if (peaks.length === 1) {
    return Buffer.isBuffer(peaks[0]) ? peaks[0] : Buffer.from(peaks[0], 'hex');
  }
  let accum = Buffer.isBuffer(peaks[0]) ? peaks[0] : Buffer.from(peaks[0], 'hex');
  for (let i = 1; i < peaks.length; i++) {
    const nextPeak = Buffer.isBuffer(peaks[i]) ? peaks[i] : Buffer.from(peaks[i], 'hex');
    accum = blake3(Buffer.concat([accum, nextPeak]));
  }
  return accum;
}

export class MerkleMountainRange {
  constructor() {
    this.leaves = []; // array of leaf buffers
    this.nodes = [];  // flat array of all node hashes
    this.peaks = [];  // array of peak buffers
  }

  get leafCount() {
    return this.leaves.length;
  }

  append(leafData) {
    const leafHash = blake3(Buffer.isBuffer(leafData) ? leafData : Buffer.from(leafData));
    const leafIndex = this.leaves.length;
    this.leaves.push(leafHash);

    // Carry-over merging
    let currentHash = leafHash;
    let height = 0;
    let totalLeaves = this.leaves.length;

    // Determine carry-over
    while ((totalLeaves & (1 << height)) === 0 && height < 32) {
      if (this.peaks.length > 0) {
        const leftHash = this.peaks.pop();
        currentHash = mmrParentHash(leftHash, currentHash);
      }
      height++;
    }
    this.peaks.push(currentHash);
    return leafIndex;
  }

  getRoot() {
    return bagPeaks(this.peaks);
  }

  getRootHex() {
    return this.getRoot().toString('hex');
  }

  generateInclusionProof(leafIndex) {
    if (leafIndex < 0 || leafIndex >= this.leaves.length) {
      throw new Error('Leaf index out of bounds');
    }
    return {
      leafIndex,
      leafHash: this.leaves[leafIndex].toString('hex'),
      peaks: this.peaks.map((p) => p.toString('hex')),
      totalLeaves: this.leaves.length,
      root: this.getRootHex(),
    };
  }

  verifyInclusionProof(proof) {
    if (!proof || proof.leafIndex >= proof.totalLeaves) return false;
    const peakHash = bagPeaks(proof.peaks);
    return peakHash.toString('hex') === proof.root;
  }

  generateBatchProof(leafIndices) {
    return {
      leafIndices,
      leafHashes: leafIndices.map((i) => this.leaves[i].toString('hex')),
      peaks: this.peaks.map((p) => p.toString('hex')),
      totalLeaves: this.leaves.length,
      root: this.getRootHex(),
    };
  }

  verifyBatchProof(proof) {
    if (!proof || !proof.leafIndices || proof.leafIndices.length === 0) return false;
    const peakHash = bagPeaks(proof.peaks);
    return peakHash.toString('hex') === proof.root;
  }
}
