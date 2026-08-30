// Pure JavaScript BLAKE3 cryptographic hash implementation
// Matches official BLAKE3 C / Rust reference implementation

const IV = new Uint32Array([
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
  0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
]);

const MSG_PERMUTATION = [
  2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8
];

const CHUNK_START = 1;
const CHUNK_END = 2;
const PARENT = 4;
const ROOT = 8;
const KEYED_HASH = 16;
const DERIVE_KEY_CONTEXT = 32;
const DERIVE_KEY_MATERIAL = 64;

function rotr(w, c) {
  return (w >>> c) | (w << (32 - c));
}

function g(state, a, b, c, d, mx, my) {
  state[a] = (state[a] + state[b] + mx) >>> 0;
  state[d] = rotr(state[d] ^ state[a], 16);
  state[c] = (state[c] + state[d]) >>> 0;
  state[b] = rotr(state[b] ^ state[c], 12);
  state[a] = (state[a] + state[b] + my) >>> 0;
  state[d] = rotr(state[d] ^ state[a], 8);
  state[c] = (state[c] + state[d]) >>> 0;
  state[b] = rotr(state[b] ^ state[c], 7);
}

function round(state, m) {
  g(state, 0, 4, 8, 12, m[0], m[1]);
  g(state, 1, 5, 9, 13, m[2], m[3]);
  g(state, 2, 6, 10, 14, m[4], m[5]);
  g(state, 3, 7, 11, 15, m[6], m[7]);
  g(state, 0, 5, 10, 15, m[8], m[9]);
  g(state, 1, 6, 11, 12, m[10], m[11]);
  g(state, 2, 7, 8, 13, m[12], m[13]);
  g(state, 3, 4, 9, 14, m[14], m[15]);
}

function permute(m) {
  const permuted = new Uint32Array(16);
  for (let i = 0; i < 16; i++) {
    permuted[i] = m[MSG_PERMUTATION[i]];
  }
  return permuted;
}

function compress(cv, blockWords, blockLen, counter, flags) {
  const state = new Uint32Array(16);
  state.set(cv, 0);
  state.set(IV, 8);
  state[12] = counter & 0xffffffff;
  state[13] = Math.floor(counter / 0x100000000) & 0xffffffff;
  state[14] = blockLen;
  state[15] = flags;

  let m = blockWords;
  for (let r = 0; r < 7; r++) {
    round(state, m);
    m = permute(m);
  }

  const out = new Uint32Array(8);
  for (let i = 0; i < 8; i++) {
    out[i] = state[i] ^ state[i + 8];
  }
  return out;
}

function wordsFromBytes(buf, offset, len) {
  const words = new Uint32Array(16);
  const fullWords = Math.floor(len / 4);
  for (let i = 0; i < fullWords; i++) {
    words[i] = buf.readUInt32LE(offset + i * 4);
  }
  const rem = len % 4;
  if (rem > 0) {
    let val = 0;
    const base = offset + fullWords * 4;
    for (let j = 0; j < rem; j++) {
      val |= (buf[base + j] << (j * 8));
    }
    words[fullWords] = val >>> 0;
  }
  return words;
}

class ChunkState {
  constructor(keyWords, chunkCounter, flags) {
    this.cv = new Uint32Array(keyWords);
    this.chunkCounter = chunkCounter;
    this.block = Buffer.alloc(64);
    this.blockLen = 0;
    this.blocksCompressed = 0;
    this.flags = flags;
  }

  len() {
    return 64 * this.blocksCompressed + this.blockLen;
  }

  startFlag() {
    return this.blocksCompressed === 0 ? CHUNK_START : 0;
  }

  update(input) {
    let offset = 0;
    while (offset < input.length) {
      if (this.blockLen === 64) {
        const blockWords = wordsFromBytes(this.block, 0, 64);
        this.cv = compress(this.cv, blockWords, 64, this.chunkCounter, this.flags | this.startFlag());
        this.blocksCompressed += 1;
        this.block.fill(0);
        this.blockLen = 0;
      }
      const take = Math.min(64 - this.blockLen, input.length - offset);
      input.copy(this.block, this.blockLen, offset, offset + take);
      this.blockLen += take;
      offset += take;
    }
  }

  output(isRoot) {
    const blockWords = wordsFromBytes(this.block, 0, this.blockLen);
    const flags = this.flags | this.startFlag() | CHUNK_END | (isRoot ? ROOT : 0);
    return compress(this.cv, blockWords, this.blockLen, this.chunkCounter, flags);
  }
}

function parentCv(leftCv, rightCv, keyWords, flags) {
  const blockWords = new Uint32Array(16);
  blockWords.set(leftCv, 0);
  blockWords.set(rightCv, 8);
  return compress(keyWords, blockWords, 64, 0, flags | PARENT);
}

export class Blake3Hasher {
  constructor(keyWords = IV, flags = 0) {
    this.keyWords = new Uint32Array(keyWords);
    this.flags = flags;
    this.chunkState = new ChunkState(this.keyWords, 0, this.flags);
    this.stack = [];
  }

  update(data) {
    const buf = Buffer.isBuffer(data) ? data : Buffer.from(typeof data === 'string' ? data : new Uint8Array(data));
    let offset = 0;
    while (offset < buf.length) {
      if (this.chunkState.len() === 1024) {
        const chunkCv = this.chunkState.output(false);
        const totalChunks = this.chunkState.chunkCounter + 1;
        this.addChunkCv(chunkCv, totalChunks);
        this.chunkState = new ChunkState(this.keyWords, totalChunks, this.flags);
      }
      const take = Math.min(1024 - this.chunkState.len(), buf.length - offset);
      this.chunkState.update(buf.subarray(offset, offset + take));
      offset += take;
    }
    return this;
  }

  addChunkCv(newCv, totalChunks) {
    let rightCv = newCv;
    while ((totalChunks & 1) === 0) {
      const leftCv = this.stack.pop();
      rightCv = parentCv(leftCv, rightCv, this.keyWords, this.flags);
      totalChunks >>>= 1;
    }
    this.stack.push(rightCv);
  }

  digest() {
    let nodeCv = this.chunkState.output(this.stack.length === 0);
    let totalChunks = this.chunkState.chunkCounter;
    for (let i = this.stack.length - 1; i >= 0; i--) {
      const isRoot = (i === 0);
      const flags = this.flags | PARENT | (isRoot ? ROOT : 0);
      const leftCv = this.stack[i];
      const blockWords = new Uint32Array(16);
      blockWords.set(leftCv, 0);
      blockWords.set(nodeCv, 8);
      nodeCv = compress(this.keyWords, blockWords, 64, 0, flags);
    }
    const res = Buffer.alloc(32);
    for (let i = 0; i < 8; i++) {
      res.writeUInt32LE(nodeCv[i], i * 4);
    }
    return res;
  }
}

export function blake3(data) {
  const hasher = new Blake3Hasher();
  hasher.update(data);
  return hasher.digest();
}

export function blake3Hex(data) {
  return blake3(data).toString('hex');
}