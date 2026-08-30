import crypto from 'node:crypto';
import { blake3, blake3Hex } from './blake3.mjs';

export const NODE_ID_DOMAIN = Buffer.from('Rivun-NODE-ID-v1', 'utf8');
export const SIGN_HINT_DOMAIN = Buffer.from('Rivun-SIGN-HINT-v1', 'utf8');
export const POA_DIGEST_DOMAIN = Buffer.from('Rivun-POA-DIGEST-v1', 'utf8');
export const POA_SIGNATURE_DOMAIN = Buffer.from('Rivun-POA-SIGNATURE-v1', 'utf8');
export const POA_VALIDATOR_SET_SIGNATURE_DOMAIN = Buffer.from('Rivun-POA-VALIDATOR-SET-v1', 'utf8');
export const BLINDED_COMMITMENT_DOMAIN = Buffer.from('Rivun-BLINDED-COMMITMENT-v1', 'utf8');
export const BLINDED_RECEIPT_DOMAIN = Buffer.from('Rivun-BLINDED-RECEIPT-v1', 'utf8');
export const BATCH_SEAL_DOMAIN = Buffer.from('Rivun-BATCH-SEAL-v1', 'utf8');
export const PROVENANCE_CHAIN_DOMAIN = Buffer.from('Rivun-PROVENANCE-CHAIN-v1', 'utf8');

export function formatUuid(buf) {
  const hex = Buffer.from(buf).toString('hex');
  return hex.slice(0, 8) + '-' + hex.slice(8, 12) + '-' + hex.slice(12, 16) + '-' + hex.slice(16, 20) + '-' + hex.slice(20, 32);
}

export function parseUuid(uuidStr) {
  const clean = uuidStr.replace(/-/g, '');
  if (clean.length !== 32) {
    throw new Error('Invalid UUID length: ' + uuidStr);
  }
  return Buffer.from(clean, 'hex');
}

export function nodeIdFromPublicKey(publicKeyBytes) {
  const pkBuf = Buffer.isBuffer(publicKeyBytes) ? publicKeyBytes : Buffer.from(publicKeyBytes);
  const toHash = Buffer.concat([NODE_ID_DOMAIN, pkBuf]);
  const hash = blake3(toHash);
  const idBuf = Buffer.alloc(16);
  hash.copy(idBuf, 0, 0, 16);
  idBuf[6] = (idBuf[6] & 0x0f) | 0x80; // UUID version 8
  idBuf[8] = (idBuf[8] & 0x3f) | 0x80; // RFC 9562 variant
  return formatUuid(idBuf);
}

export function signatureHint(signatureBytes) {
  const sigBuf = Buffer.isBuffer(signatureBytes) ? signatureBytes : Buffer.from(signatureBytes);
  const toHash = Buffer.concat([SIGN_HINT_DOMAIN, sigBuf]);
  const hash = blake3(toHash);
  return hash.subarray(0, 8);
}

export function domainMessage(domain, message) {
  const domBuf = Buffer.isBuffer(domain) ? domain : Buffer.from(domain, 'utf8');
  const msgBuf = Buffer.isBuffer(message) ? message : Buffer.from(message);
  return Buffer.concat([domBuf, Buffer.from([0]), msgBuf]);
}

export class Keypair {
  constructor(privateKeyObject, publicKeyObject, rawPublicKey, rawPrivateKey) {
    this._privateKey = privateKeyObject;
    this._publicKey = publicKeyObject;
    this._rawPublicKey = rawPublicKey;
    this._rawPrivateKey = rawPrivateKey;
    this._nodeId = nodeIdFromPublicKey(rawPublicKey);
  }

  static generate() {
    const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
    const rawPublic = publicKey.export({ type: 'spki', format: 'der' }).subarray(-32);
    const rawPrivate = privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32);
    return new Keypair(privateKey, publicKey, rawPublic, rawPrivate);
  }

  static fromKeyFile(keyFile) {
    if (!keyFile || keyFile.schema_version !== 1) {
      throw new Error('Unsupported schema version');
    }
    const pubBytes = Buffer.from(keyFile.public_key || '', 'base64url');
    const secBytes = Buffer.from(keyFile.secret_key || '', 'base64url');
    if (pubBytes.length !== 32 || secBytes.length !== 32) {
      throw new Error('Invalid key length: public and secret keys must be 32 bytes');
    }
    const expectedNodeId = nodeIdFromPublicKey(pubBytes);
    if (keyFile.node_id && keyFile.node_id !== expectedNodeId) {
      throw new Error('Node ID mismatch: ' + keyFile.node_id + ' != ' + expectedNodeId);
    }
    const privDer = Buffer.concat([
      Buffer.from('302e020100300506032b657004220420', 'hex'),
      secBytes,
    ]);
    const pubDer = Buffer.concat([
      Buffer.from('302a300506032b6570032100', 'hex'),
      pubBytes,
    ]);
    const privateKey = crypto.createPrivateKey({ key: privDer, format: 'der', type: 'pkcs8' });
    const publicKey = crypto.createPublicKey({ key: pubDer, format: 'der', type: 'spki' });
    return new Keypair(privateKey, publicKey, pubBytes, secBytes);
  }

  get nodeId() {
    return this._nodeId;
  }

  get publicKeyBytes() {
    return this._rawPublicKey;
  }

  get secretKeyBytes() {
    return this._rawPrivateKey;
  }

  sign(message) {
    const msgBuf = Buffer.isBuffer(message) ? message : Buffer.from(message);
    return crypto.sign(null, msgBuf, this._privateKey);
  }

  signDomainMessage(domain, message) {
    const msg = domainMessage(domain, message);
    return this.sign(msg);
  }

  getVerifyingKey() {
    return new PublicKey(this._publicKey, this._rawPublicKey);
  }

  toKeyFile() {
    return {
      schema_version: 1,
      node_id: this._nodeId,
      public_key: this._rawPublicKey.toString('base64url'),
      secret_key: this._rawPrivateKey.toString('base64url'),
    };
  }
}

export class PublicKey {
  constructor(publicKeyObject, rawPublicKey) {
    this._publicKey = publicKeyObject;
    this._rawPublicKey = rawPublicKey;
    this._nodeId = nodeIdFromPublicKey(rawPublicKey);
  }

  static fromBytes(rawPublicKey) {
    const pubBuf = Buffer.isBuffer(rawPublicKey) ? rawPublicKey : Buffer.from(rawPublicKey);
    if (pubBuf.length !== 32) {
      throw new Error('Ed25519 public key must be exactly 32 bytes');
    }
    const pubDer = Buffer.concat([
      Buffer.from('302a300506032b6570032100', 'hex'),
      pubBuf,
    ]);
    const publicKey = crypto.createPublicKey({ key: pubDer, format: 'der', type: 'spki' });
    return new PublicKey(publicKey, pubBuf);
  }

  get nodeId() {
    return this._nodeId;
  }

  get bytes() {
    return this._rawPublicKey;
  }

  verify(message, signature) {
    try {
      const msgBuf = Buffer.isBuffer(message) ? message : Buffer.from(message);
      const sigBuf = Buffer.isBuffer(signature) ? signature : Buffer.from(signature);
      return crypto.verify(null, msgBuf, this._publicKey, sigBuf);
    } catch {
      return false;
    }
  }

  verifyDomainMessage(domain, message, signature) {
    const msg = domainMessage(domain, message);
    return this.verify(msg, signature);
  }
}

export class BlindedCommitment {
  static generateBlindingFactor() {
    return crypto.randomBytes(32);
  }

  static commit(domain, value, blindingFactor) {
    const domBuf = Buffer.isBuffer(domain) ? domain : Buffer.from(domain, 'utf8');
    const valBuf = Buffer.isBuffer(value) ? value : Buffer.from(value, 'utf8');
    const blindBuf = Buffer.isBuffer(blindingFactor) ? blindingFactor : Buffer.from(blindingFactor);

    const toHash = Buffer.concat([
      BLINDED_COMMITMENT_DOMAIN,
      domBuf,
      Buffer.from([0]),
      valBuf,
      Buffer.from([0]),
      blindBuf,
    ]);
    return blake3(toHash);
  }

  static verify(commitment, domain, value, blindingFactor) {
    const expected = this.commit(domain, value, blindingFactor);
    const commBuf = Buffer.isBuffer(commitment) ? commitment : Buffer.from(commitment);
    return crypto.timingSafeEqual(commBuf, expected);
  }
}

export class BlindedReceiptCommitment {
  static commit(publicPayload, privateReceipt, blindingFactor) {
    const pubBuf = Buffer.isBuffer(publicPayload) ? publicPayload : Buffer.from(publicPayload, 'utf8');
    const privBuf = Buffer.isBuffer(privateReceipt) ? privateReceipt : Buffer.from(privateReceipt, 'utf8');
    const blindBuf = Buffer.isBuffer(blindingFactor) ? blindingFactor : Buffer.from(blindingFactor);

    const toHash = Buffer.concat([
      BLINDED_RECEIPT_DOMAIN,
      pubBuf,
      Buffer.from([0]),
      privBuf,
      Buffer.from([0]),
      blindBuf,
    ]);
    return blake3(toHash);
  }

  static verify(commitment, publicPayload, privateReceipt, blindingFactor) {
    const expected = this.commit(publicPayload, privateReceipt, blindingFactor);
    const commBuf = Buffer.isBuffer(commitment) ? commitment : Buffer.from(commitment);
    return crypto.timingSafeEqual(commBuf, expected);
  }
}

export function encryptChaCha20Poly1305(key, nonce, plaintext, aad = Buffer.alloc(0)) {
  const cipher = crypto.createCipheriv('chacha20-poly1305', key, nonce, { authTagLength: 16 });
  if (aad.length > 0) cipher.setAAD(aad);
  const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final()]);
  const tag = cipher.getAuthTag();
  return { ciphertext, tag };
}

export function decryptChaCha20Poly1305(key, nonce, ciphertext, tag, aad = Buffer.alloc(0)) {
  const decipher = crypto.createDecipheriv('chacha20-poly1305', key, nonce, { authTagLength: 16 });
  if (aad.length > 0) decipher.setAAD(aad);
  decipher.setAuthTag(tag);
  return Buffer.concat([decipher.update(ciphertext), decipher.final()]);
}
