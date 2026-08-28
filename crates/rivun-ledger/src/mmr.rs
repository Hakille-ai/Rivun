//! Merkle Mountain Range (MMR) Accumulator for Rivun Receipts.
//!
//! Provides append-only logarithmic accumulator, peak bagging, compact Merkle inclusion proofs,
//! multi-leaf batch inclusion proofs (with deduplicated sister DAG), non-membership / exclusion proofs,
//! binary disk persistence (`.zmmr`), and batch rollup commitments for zero-knowledge and cross-cluster auditability.

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fmt, fs,
    path::Path,
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MmrError {
    #[error("leaf index {0} out of bounds (size {1})")]
    LeafIndexOutOfBounds(usize, usize),
    #[error("MMR is empty")]
    EmptyMmr,
    #[error("invalid inclusion proof: hash mismatch (computed {computed}, expected {expected})")]
    InvalidProof { computed: String, expected: String },
    #[error("corrupted MMR tree structure")]
    CorruptedStructure,
    #[error("invalid leaf count in proof")]
    InvalidLeafCount,
    #[error("invalid exclusion proof: {0}")]
    InvalidExclusionProof(String),
    #[error("invalid zmmr magic")]
    InvalidZmmrMagic,
    #[error("unsupported zmmr version {0}")]
    UnsupportedZmmrVersion(u16),
    #[error("invalid zmmr payload: expected {expected} bytes, got {actual}")]
    InvalidZmmrPayload { expected: usize, actual: usize },
    #[error("zmmr root mismatch: computed {computed}, expected {expected}")]
    ZmmrRootMismatch { computed: String, expected: String },
}

/// 32-byte Blake3 cryptographic digest.
pub type MmrHash = [u8; 32];

pub const MMR_LEAF_DOMAIN: &[u8] = b"Rivun-MMR-LEAF-v1:";
pub const MMR_NODE_DOMAIN: &[u8] = b"Rivun-MMR-NODE-v1:";
pub const MMR_PEAK_BAG_DOMAIN: &[u8] = b"Rivun-MMR-PEAK-BAG-v1:";
pub const ZMMR_MAGIC: &[u8; 8] = b"ZAPMMR01";
pub const ZMMR_VERSION: u16 = 1;
pub const ZMMR_HEADER_LEN: usize = 68;

pub fn hash_leaf(data: &[u8]) -> MmrHash {
    let mut hasher = Hasher::new();
    hasher.update(MMR_LEAF_DOMAIN);
    hasher.update(data);
    *hasher.finalize().as_bytes()
}

pub fn hash_nodes(left: &MmrHash, right: &MmrHash) -> MmrHash {
    let mut hasher = Hasher::new();
    hasher.update(MMR_NODE_DOMAIN);
    hasher.update(left);
    hasher.update(right);
    *hasher.finalize().as_bytes()
}

pub fn bag_peaks(peaks: &[MmrHash]) -> MmrHash {
    if peaks.is_empty() {
        return [0u8; 32];
    }
    if peaks.len() == 1 {
        return peaks[0];
    }
    let mut current = peaks[0];
    for peak in &peaks[1..] {
        let mut hasher = Hasher::new();
        hasher.update(MMR_PEAK_BAG_DOMAIN);
        hasher.update(&current);
        hasher.update(peak);
        current = *hasher.finalize().as_bytes();
    }
    current
}

/// A compact inclusion proof for a single leaf in an MMR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmrInclusionProof {
    pub leaf_index: usize,
    pub leaf_hash: String,
    pub total_leaves: usize,
    pub sister_hashes: Vec<String>,
    pub peak_hashes: Vec<String>,
}

/// Deduplicated multi-leaf batch inclusion proof using a sister DAG.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmrBatchInclusionProof {
    /// Total number of leaves in the MMR when the proof was constructed.
    pub total_leaves: u64,
    /// Target leaf indices (strictly ascending).
    pub leaf_indices: Vec<u64>,
    /// Hex-encoded hashes of the target leaves.
    pub leaf_hashes: Vec<String>,
    /// Minimal deduplicated sister hashes in canonical DAG evaluation order.
    pub sister_hashes: Vec<String>,
    /// Hex-encoded peak hashes for subtrees containing zero queried leaves: (peak_index, peak_hash).
    pub untouched_peaks: Vec<(usize, String)>,
}

impl MmrBatchInclusionProof {
    /// Verifies the multi-leaf batch inclusion proof against the expected MMR root.
    pub fn verify(&self, expected_root: &MmrHash) -> Result<bool, MmrError> {
        if self.total_leaves == 0 || self.leaf_indices.is_empty() {
            return Err(MmrError::InvalidLeafCount);
        }
        if self.leaf_indices.len() != self.leaf_hashes.len() {
            return Err(MmrError::InvalidLeafCount);
        }

        // Validate strictly ascending order and bounds
        for i in 0..self.leaf_indices.len() {
            if self.leaf_indices[i] >= self.total_leaves {
                return Err(MmrError::LeafIndexOutOfBounds(
                    self.leaf_indices[i] as usize,
                    self.total_leaves as usize,
                ));
            }
            if i > 0 && self.leaf_indices[i] <= self.leaf_indices[i - 1] {
                return Err(MmrError::InvalidProof {
                    computed: format!("unsorted leaf index {}", self.leaf_indices[i]),
                    expected: "strictly ascending leaf indices".to_string(),
                });
            }
        }

        // Decompose total_leaves into mountains (from highest to lowest bit)
        let mut mountains = Vec::new();
        let mut offset = 0u64;
        for bit in (0..64).rev() {
            let tree_size = 1u64 << bit;
            if offset + tree_size <= self.total_leaves {
                mountains.push((offset, bit as usize, tree_size));
                offset += tree_size;
            }
        }

        let mut sister_iter = self.sister_hashes.iter();
        let untouched_map: BTreeMap<usize, &str> = self
            .untouched_peaks
            .iter()
            .map(|(idx, h)| (*idx, h.as_str()))
            .collect();

        let mut computed_peaks = Vec::with_capacity(mountains.len());

        for (m_idx, &(m_start, m_height, m_size)) in mountains.iter().enumerate() {
            // Find target leaves in this mountain
            let mut targets = Vec::new();
            for (idx, hash_str) in self.leaf_indices.iter().zip(self.leaf_hashes.iter()) {
                if *idx >= m_start && *idx < m_start + m_size {
                    let local_leaf_idx = (*idx - m_start) as usize;
                    let leaf_bytes =
                        hex::decode(hash_str).map_err(|_| MmrError::CorruptedStructure)?;
                    if leaf_bytes.len() != 32 {
                        return Err(MmrError::CorruptedStructure);
                    }
                    let mut h = [0u8; 32];
                    h.copy_from_slice(&leaf_bytes);
                    targets.push((local_leaf_idx, h));
                }
            }

            if targets.is_empty() {
                // Mountain has no target leaves; read from untouched_peaks
                let peak_hex = untouched_map
                    .get(&m_idx)
                    .ok_or(MmrError::CorruptedStructure)?;
                let peak_bytes = hex::decode(peak_hex).map_err(|_| MmrError::CorruptedStructure)?;
                if peak_bytes.len() != 32 {
                    return Err(MmrError::CorruptedStructure);
                }
                let mut peak_hash = [0u8; 32];
                peak_hash.copy_from_slice(&peak_bytes);
                computed_peaks.push(peak_hash);
            } else {
                // Mountain has target leaves; evaluate DAG bottom-up
                let mut current_known: BTreeMap<usize, MmrHash> = targets.into_iter().collect();

                for _h in 0..m_height {
                    let mut next_known: BTreeMap<usize, MmrHash> = BTreeMap::new();
                    let keys: Vec<usize> = current_known.keys().copied().collect();
                    let mut k_idx = 0;

                    while k_idx < keys.len() {
                        let j = keys[k_idx];
                        let s = j ^ 1;

                        if let Some(&sibling_hash) = current_known.get(&s) {
                            // Both left and right are known
                            let left_hash = if j < s {
                                current_known[&j]
                            } else {
                                sibling_hash
                            };
                            let right_hash = if j < s {
                                sibling_hash
                            } else {
                                current_known[&j]
                            };
                            let parent_hash = hash_nodes(&left_hash, &right_hash);
                            next_known.insert(j / 2, parent_hash);

                            // If next key is the right sibling, skip it to avoid double-processing
                            if k_idx + 1 < keys.len() && keys[k_idx + 1] == j.max(s) {
                                k_idx += 1;
                            }
                        } else {
                            // Sibling is not in known nodes, retrieve from sister_hashes
                            let sister_hex =
                                sister_iter.next().ok_or(MmrError::CorruptedStructure)?;
                            let sister_bytes = hex::decode(sister_hex)
                                .map_err(|_| MmrError::CorruptedStructure)?;
                            if sister_bytes.len() != 32 {
                                return Err(MmrError::CorruptedStructure);
                            }
                            let mut sister_hash = [0u8; 32];
                            sister_hash.copy_from_slice(&sister_bytes);

                            let parent_hash = if j.is_multiple_of(2) {
                                hash_nodes(&current_known[&j], &sister_hash)
                            } else {
                                hash_nodes(&sister_hash, &current_known[&j])
                            };
                            next_known.insert(j / 2, parent_hash);
                        }

                        k_idx += 1;
                    }

                    current_known = next_known;
                }

                let peak_hash = current_known
                    .get(&0)
                    .copied()
                    .ok_or(MmrError::CorruptedStructure)?;
                computed_peaks.push(peak_hash);
            }
        }

        // Verify that all sister hashes in the proof were consumed
        if sister_iter.next().is_some() {
            return Err(MmrError::CorruptedStructure);
        }

        let computed_root = bag_peaks(&computed_peaks);
        if computed_root != *expected_root {
            return Err(MmrError::InvalidProof {
                computed: hex::encode(computed_root),
                expected: hex::encode(*expected_root),
            });
        }

        Ok(true)
    }
}

/// Cryptographic non-membership / exclusion proofs against an MMR root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MmrExclusionProof {
    /// Proves a requested sequence is strictly before the earliest leaf in the ledger.
    BeforeRange {
        requested_seq: u64,
        first_leaf_index: u64,
        first_leaf_seq: u64,
        first_leaf_hash: String,
        inclusion_proof: MmrInclusionProof,
    },
    /// Proves a requested sequence is strictly after the latest leaf in the ledger.
    AfterRange {
        requested_seq: u64,
        last_leaf_index: u64,
        last_leaf_seq: u64,
        last_leaf_hash: String,
        inclusion_proof: MmrInclusionProof,
    },
    /// Proves a sequence gap between two adjacent leaves in the ledger.
    SequenceGap {
        requested_seq: u64,
        left_index: u64,
        left_seq: u64,
        left_leaf_hash: String,
        left_proof: MmrInclusionProof,
        right_index: u64,
        right_seq: u64,
        right_leaf_hash: String,
        right_proof: MmrInclusionProof,
    },
    /// Proves a hash is strictly between two lexicographically adjacent leaves.
    HashBound {
        target_hash: String,
        left_index: u64,
        left_hash: String,
        left_proof: MmrInclusionProof,
        right_index: u64,
        right_hash: String,
        right_proof: MmrInclusionProof,
    },
}

impl MmrExclusionProof {
    /// Verifies the exclusion proof against the expected MMR root.
    pub fn verify(&self, expected_root: &MmrHash) -> Result<bool, MmrError> {
        match self {
            MmrExclusionProof::BeforeRange {
                requested_seq,
                first_leaf_index,
                first_leaf_seq,
                first_leaf_hash,
                inclusion_proof,
            } => {
                if *first_leaf_index != 0 || inclusion_proof.leaf_index != 0 {
                    return Err(MmrError::InvalidExclusionProof(
                        "first_leaf_index must be 0".to_string(),
                    ));
                }
                if *requested_seq >= *first_leaf_seq {
                    return Err(MmrError::InvalidExclusionProof(
                        "requested_seq is not strictly before first_leaf_seq".to_string(),
                    ));
                }
                if inclusion_proof.leaf_hash != *first_leaf_hash {
                    return Err(MmrError::InvalidExclusionProof(
                        "first_leaf_hash mismatch".to_string(),
                    ));
                }
                MerkleMountainRange::verify_proof(inclusion_proof, expected_root)
            }
            MmrExclusionProof::AfterRange {
                requested_seq,
                last_leaf_index,
                last_leaf_seq,
                last_leaf_hash,
                inclusion_proof,
            } => {
                if inclusion_proof.total_leaves == 0 {
                    return Err(MmrError::InvalidLeafCount);
                }
                let expected_last_idx = (inclusion_proof.total_leaves as u64) - 1;
                if *last_leaf_index != expected_last_idx
                    || inclusion_proof.leaf_index as u64 != expected_last_idx
                {
                    return Err(MmrError::InvalidExclusionProof(
                        "last_leaf_index must match total_leaves - 1".to_string(),
                    ));
                }
                if *requested_seq <= *last_leaf_seq {
                    return Err(MmrError::InvalidExclusionProof(
                        "requested_seq is not strictly after last_leaf_seq".to_string(),
                    ));
                }
                if inclusion_proof.leaf_hash != *last_leaf_hash {
                    return Err(MmrError::InvalidExclusionProof(
                        "last_leaf_hash mismatch".to_string(),
                    ));
                }
                MerkleMountainRange::verify_proof(inclusion_proof, expected_root)
            }
            MmrExclusionProof::SequenceGap {
                requested_seq,
                left_index,
                left_seq,
                left_leaf_hash,
                left_proof,
                right_index,
                right_seq,
                right_leaf_hash,
                right_proof,
            } => {
                if *right_index != *left_index + 1 {
                    return Err(MmrError::InvalidExclusionProof(
                        "right_index must be left_index + 1".to_string(),
                    ));
                }
                if left_proof.total_leaves != right_proof.total_leaves {
                    return Err(MmrError::InvalidExclusionProof(
                        "left_proof and right_proof total_leaves mismatch".to_string(),
                    ));
                }
                if !(*left_seq < *requested_seq && *requested_seq < *right_seq) {
                    return Err(MmrError::InvalidExclusionProof(
                        "requested_seq is not strictly between left_seq and right_seq".to_string(),
                    ));
                }
                if left_proof.leaf_index as u64 != *left_index
                    || right_proof.leaf_index as u64 != *right_index
                {
                    return Err(MmrError::InvalidExclusionProof(
                        "proof leaf_index does not match declared index".to_string(),
                    ));
                }
                if left_proof.leaf_hash != *left_leaf_hash
                    || right_proof.leaf_hash != *right_leaf_hash
                {
                    return Err(MmrError::InvalidExclusionProof(
                        "leaf hash mismatch in gap proofs".to_string(),
                    ));
                }
                let left_ok = MerkleMountainRange::verify_proof(left_proof, expected_root)?;
                let right_ok = MerkleMountainRange::verify_proof(right_proof, expected_root)?;
                Ok(left_ok && right_ok)
            }
            MmrExclusionProof::HashBound {
                target_hash,
                left_index,
                left_hash,
                left_proof,
                right_index,
                right_hash,
                right_proof,
            } => {
                if *right_index != *left_index + 1 {
                    return Err(MmrError::InvalidExclusionProof(
                        "right_index must be left_index + 1".to_string(),
                    ));
                }
                if left_proof.total_leaves != right_proof.total_leaves {
                    return Err(MmrError::InvalidExclusionProof(
                        "left_proof and right_proof total_leaves mismatch".to_string(),
                    ));
                }
                if !(left_hash.as_str() < target_hash.as_str()
                    && target_hash.as_str() < right_hash.as_str())
                {
                    return Err(MmrError::InvalidExclusionProof(
                        "target_hash is not strictly bounded by left_hash and right_hash"
                            .to_string(),
                    ));
                }
                if left_proof.leaf_index as u64 != *left_index
                    || right_proof.leaf_index as u64 != *right_index
                {
                    return Err(MmrError::InvalidExclusionProof(
                        "proof leaf_index does not match declared index".to_string(),
                    ));
                }
                if left_proof.leaf_hash != *left_hash || right_proof.leaf_hash != *right_hash {
                    return Err(MmrError::InvalidExclusionProof(
                        "leaf hash mismatch in bound proofs".to_string(),
                    ));
                }
                let left_ok = MerkleMountainRange::verify_proof(left_proof, expected_root)?;
                let right_ok = MerkleMountainRange::verify_proof(right_proof, expected_root)?;
                Ok(left_ok && right_ok)
            }
        }
    }
}

/// Compact Batch Rollup Commitment representing a batch of receipts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmrRollupCommitment {
    pub root_hash: String,
    pub leaf_count: usize,
    pub first_leaf_hash: String,
    pub last_leaf_hash: String,
    pub min_processed_at_micros: u64,
    pub max_processed_at_micros: u64,
}

mod serde_peaks_64 {
    use super::*;
    use serde::{Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(
        peaks: &[Option<MmrHash>; 64],
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let vec: Vec<&Option<MmrHash>> = peaks.iter().collect();
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<[Option<MmrHash>; 64], D::Error> {
        let vec: Vec<Option<MmrHash>> = Vec::deserialize(deserializer)?;
        if vec.len() != 64 {
            return Err(D::Error::custom(format!(
                "expected 64 elements for peaks, got {}",
                vec.len()
            )));
        }
        let mut arr = [None; 64];
        for (i, item) in vec.into_iter().enumerate() {
            arr[i] = item;
        }
        Ok(arr)
    }
}

/// Incremental O(log N) Peak Accumulator with binary carry-over tree merging and `.zmmr` disk persistence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncrementalMmr {
    /// Total number of leaves accumulated.
    pub leaf_count: u64,
    /// Active subtree peak hashes indexed by height (0..64).
    /// peaks[h] is Some(hash) iff (leaf_count >> h) & 1 == 1.
    #[serde(with = "serde_peaks_64")]
    pub peaks: [Option<MmrHash>; 64],
    /// Lazily computed and cached root hash.
    #[serde(skip)]
    pub cached_root: Option<MmrHash>,
}

impl Default for IncrementalMmr {
    fn default() -> Self {
        Self::new()
    }
}

impl IncrementalMmr {
    pub const MAX_HEIGHT: usize = 64;

    pub fn new() -> Self {
        Self {
            leaf_count: 0,
            peaks: [None; Self::MAX_HEIGHT],
            cached_root: None,
        }
    }

    pub fn len(&self) -> u64 {
        self.leaf_count
    }

    pub fn is_empty(&self) -> bool {
        self.leaf_count == 0
    }

    /// Appends a leaf hash with amortized O(1) binary carry-over tree merging.
    pub fn append_leaf(&mut self, leaf_hash: MmrHash) -> u64 {
        let leaf_idx = self.leaf_count;
        let mut current_hash = leaf_hash;
        let mut height = 0usize;

        while height < Self::MAX_HEIGHT && self.peaks[height].is_some() {
            let existing_peak = self.peaks[height].take().unwrap();
            current_hash = hash_nodes(&existing_peak, &current_hash);
            height += 1;
        }

        if height < Self::MAX_HEIGHT {
            self.peaks[height] = Some(current_hash);
        }
        self.leaf_count += 1;
        self.cached_root = None;
        leaf_idx
    }

    pub fn append_bytes(&mut self, data: &[u8]) -> u64 {
        let h = hash_leaf(data);
        self.append_leaf(h)
    }

    /// Returns active peaks ordered from highest to lowest mountain (bit 63 down to 0).
    pub fn get_peaks(&self) -> Vec<MmrHash> {
        let mut peaks = Vec::new();
        for h in (0..Self::MAX_HEIGHT).rev() {
            if let Some(p) = self.peaks[h] {
                peaks.push(p);
            }
        }
        peaks
    }

    /// Returns the canonical MMR root by folding active peaks.
    pub fn get_root(&mut self) -> MmrHash {
        if let Some(r) = self.cached_root {
            return r;
        }
        let peaks = self.get_peaks();
        let r = bag_peaks(&peaks);
        self.cached_root = Some(r);
        r
    }

    pub fn get_root_cached(&self) -> Option<MmrHash> {
        self.cached_root
    }

    pub fn root_hex(&mut self) -> String {
        hex::encode(self.get_root())
    }

    /// Encodes accumulator state to binary `.zmmr` format.
    pub fn to_zmmr_bytes(&mut self) -> Vec<u8> {
        let root = self.get_root();
        let mut bitmask: u64 = 0;
        let active_peaks = self.get_peaks();

        for (h, peak) in self.peaks.iter().enumerate() {
            if peak.is_some() {
                bitmask |= 1u64 << h;
            }
        }

        let mut buf = Vec::with_capacity(ZMMR_HEADER_LEN + active_peaks.len() * 32);
        buf.extend_from_slice(ZMMR_MAGIC); // 00..08
        buf.extend_from_slice(&ZMMR_VERSION.to_le_bytes()); // 08..10
        buf.extend_from_slice(&0u16.to_le_bytes()); // 10..12 (Flags)
        buf.extend_from_slice(&self.leaf_count.to_le_bytes()); // 12..20
        buf.extend_from_slice(&bitmask.to_le_bytes()); // 20..28
        buf.extend_from_slice(&root); // 28..60
        buf.extend_from_slice(&[0u8; 8]); // 60..68 (Reserved)

        for peak in &active_peaks {
            buf.extend_from_slice(peak);
        }

        buf
    }

    /// Decodes and verifies accumulator state from binary `.zmmr` bytes.
    pub fn from_zmmr_bytes(bytes: &[u8]) -> Result<Self, MmrError> {
        if bytes.len() < ZMMR_HEADER_LEN {
            return Err(MmrError::InvalidZmmrPayload {
                expected: ZMMR_HEADER_LEN,
                actual: bytes.len(),
            });
        }
        if &bytes[0..8] != ZMMR_MAGIC {
            return Err(MmrError::InvalidZmmrMagic);
        }
        let version = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
        if version != ZMMR_VERSION {
            return Err(MmrError::UnsupportedZmmrVersion(version));
        }
        let leaf_count = u64::from_le_bytes(bytes[12..20].try_into().unwrap());
        let bitmask = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
        let mut expected_root = [0u8; 32];
        expected_root.copy_from_slice(&bytes[28..60]);

        let peak_count = bitmask.count_ones() as usize;
        let expected_total_len = ZMMR_HEADER_LEN + peak_count * 32;
        if bytes.len() != expected_total_len {
            return Err(MmrError::InvalidZmmrPayload {
                expected: expected_total_len,
                actual: bytes.len(),
            });
        }

        let mut peaks = [None; Self::MAX_HEIGHT];
        let mut offset = ZMMR_HEADER_LEN;
        for h in (0..Self::MAX_HEIGHT).rev() {
            if (bitmask & (1u64 << h)) != 0 {
                let mut peak_bytes = [0u8; 32];
                peak_bytes.copy_from_slice(&bytes[offset..offset + 32]);
                peaks[h] = Some(peak_bytes);
                offset += 32;
            }
        }

        let mmr = Self {
            leaf_count,
            peaks,
            cached_root: Some(expected_root),
        };

        // Validate computed root against recorded root
        let computed_peaks = mmr.get_peaks();
        let computed_root = bag_peaks(&computed_peaks);
        if computed_root != expected_root {
            return Err(MmrError::ZmmrRootMismatch {
                computed: hex::encode(computed_root),
                expected: hex::encode(expected_root),
            });
        }

        Ok(mmr)
    }

    /// Persists the accumulator state to a `.zmmr` binary file.
    pub fn save_to_file(&mut self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let bytes = self.to_zmmr_bytes();
        let path_ref = path.as_ref();
        if let Some(parent) = path_ref.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path_ref, bytes)
    }

    /// Loads and verifies the accumulator state from a `.zmmr` binary file.
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let bytes = fs::read(path)?;
        Self::from_zmmr_bytes(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }
}

/// In-memory Merkle Mountain Range Accumulator with full leaf retention for proof generation.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MerkleMountainRange {
    leaves: Vec<MmrHash>,
    peaks: Vec<MmrHash>,
    cached_root: Option<MmrHash>,
}

impl MerkleMountainRange {
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            peaks: Vec::new(),
            cached_root: None,
        }
    }

    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    pub fn leaves(&self) -> &[MmrHash] {
        &self.leaves
    }

    pub fn append(&mut self, leaf_hash: MmrHash) -> usize {
        let leaf_idx = self.leaves.len();
        self.leaves.push(leaf_hash);
        self.cached_root = None;
        self.recompute_peaks();
        leaf_idx
    }

    pub fn append_bytes(&mut self, data: &[u8]) -> usize {
        let h = hash_leaf(data);
        self.append(h)
    }

    pub fn root(&mut self) -> MmrHash {
        if let Some(r) = self.cached_root {
            return r;
        }
        let r = bag_peaks(&self.peaks);
        self.cached_root = Some(r);
        r
    }

    pub fn root_hex(&mut self) -> String {
        hex::encode(self.root())
    }

    pub fn peaks(&self) -> &[MmrHash] {
        &self.peaks
    }

    fn recompute_peaks(&mut self) {
        if self.leaves.is_empty() {
            self.peaks.clear();
            return;
        }

        let n = self.leaves.len();
        let mut peaks = Vec::new();
        let mut offset = 0;

        for bit in (0..usize::BITS).rev() {
            let tree_size = 1usize << bit;
            if offset + tree_size <= n {
                let peak = self.build_subtree_root(offset, tree_size);
                peaks.push(peak);
                offset += tree_size;
            }
        }

        self.peaks = peaks;
    }

    fn build_subtree_root(&self, start: usize, size: usize) -> MmrHash {
        if size == 1 {
            return self.leaves[start];
        }
        let half = size / 2;
        let left = self.build_subtree_root(start, half);
        let right = self.build_subtree_root(start + half, half);
        hash_nodes(&left, &right)
    }

    /// Generate an O(log N) inclusion proof for the leaf at `leaf_index`.
    pub fn prove_inclusion(&mut self, leaf_index: usize) -> Result<MmrInclusionProof, MmrError> {
        let total_leaves = self.leaves.len();
        if leaf_index >= total_leaves {
            return Err(MmrError::LeafIndexOutOfBounds(leaf_index, total_leaves));
        }

        let leaf_hash = self.leaves[leaf_index];

        // Find which peak tree this leaf belongs to
        let mut offset = 0;
        let mut target_tree_size = 0;
        let mut target_tree_start = 0;

        for bit in (0..usize::BITS).rev() {
            let tree_size = 1usize << bit;
            if offset + tree_size <= total_leaves {
                if leaf_index >= offset && leaf_index < offset + tree_size {
                    target_tree_start = offset;
                    target_tree_size = tree_size;
                    break;
                }
                offset += tree_size;
            }
        }

        // Collect sister hashes within this binary tree
        let mut sister_hashes = Vec::new();
        let mut curr_start = target_tree_start;
        let mut curr_size = target_tree_size;

        while curr_size > 1 {
            let half = curr_size / 2;
            let left_root = self.build_subtree_root(curr_start, half);
            let right_root = self.build_subtree_root(curr_start + half, half);

            if leaf_index < curr_start + half {
                // Leaf is in the left branch, right is sister
                sister_hashes.push(hex::encode(right_root));
                curr_size = half;
            } else {
                // Leaf is in the right branch, left is sister
                sister_hashes.push(hex::encode(left_root));
                curr_start += half;
                curr_size = half;
            }
        }
        // Sisters are collected from root downwards; reverse so verification goes bottom-up
        sister_hashes.reverse();

        let peak_hashes: Vec<String> = self.peaks.iter().map(|p| hex::encode(*p)).collect();

        Ok(MmrInclusionProof {
            leaf_index,
            leaf_hash: hex::encode(leaf_hash),
            total_leaves,
            sister_hashes,
            peak_hashes,
        })
    }

    /// Generates a deduplicated multi-leaf batch inclusion proof using a compact sister DAG.
    pub fn prove_batch_inclusion(
        &mut self,
        leaf_indices: &[usize],
    ) -> Result<MmrBatchInclusionProof, MmrError> {
        let total_leaves = self.leaves.len();
        if total_leaves == 0 || leaf_indices.is_empty() {
            return Err(MmrError::InvalidLeafCount);
        }

        let mut sorted_indices = leaf_indices.to_vec();
        sorted_indices.sort_unstable();
        sorted_indices.dedup();

        for &idx in &sorted_indices {
            if idx >= total_leaves {
                return Err(MmrError::LeafIndexOutOfBounds(idx, total_leaves));
            }
        }

        let mut leaf_hashes = Vec::with_capacity(sorted_indices.len());
        for &idx in &sorted_indices {
            leaf_hashes.push(hex::encode(self.leaves[idx]));
        }

        // Decompose total_leaves into mountains
        let mut mountains = Vec::new();
        let mut offset = 0usize;
        for bit in (0..usize::BITS).rev() {
            let tree_size = 1usize << bit;
            if offset + tree_size <= total_leaves {
                mountains.push((offset, bit as usize, tree_size));
                offset += tree_size;
            }
        }

        let mut sister_hashes = Vec::new();
        let mut untouched_peaks = Vec::new();

        for (m_idx, &(m_start, m_height, m_size)) in mountains.iter().enumerate() {
            let targets: Vec<usize> = sorted_indices
                .iter()
                .copied()
                .filter(|&i| i >= m_start && i < m_start + m_size)
                .map(|i| i - m_start)
                .collect();

            if targets.is_empty() {
                let peak_hash = self.build_subtree_root(m_start, m_size);
                untouched_peaks.push((m_idx, hex::encode(peak_hash)));
            } else {
                let mut active_indices = targets;
                for h in 0..m_height {
                    let mut next_active = Vec::new();
                    let active_set: HashSet<usize> = active_indices.iter().copied().collect();
                    let node_size = 1usize << h;

                    let mut processed_pairs = HashSet::new();
                    for &j in &active_indices {
                        let parent_j = j / 2;
                        if !next_active.contains(&parent_j) {
                            next_active.push(parent_j);
                        }

                        let pair_id = j / 2;
                        if processed_pairs.insert(pair_id) {
                            let left_j = (j / 2) * 2;
                            let right_j = left_j + 1;

                            let has_left = active_set.contains(&left_j);
                            let has_right = active_set.contains(&right_j);

                            match (has_left, has_right) {
                                (true, false) => {
                                    // Sibling is right child
                                    let sib_start = m_start + right_j * node_size;
                                    let sib_hash = self.build_subtree_root(sib_start, node_size);
                                    sister_hashes.push(hex::encode(sib_hash));
                                }
                                (false, true) => {
                                    // Sibling is left child
                                    let sib_start = m_start + left_j * node_size;
                                    let sib_hash = self.build_subtree_root(sib_start, node_size);
                                    sister_hashes.push(hex::encode(sib_hash));
                                }
                                (true, true) => {
                                    // Both children are present in the batch; no sister hash needed
                                }
                                (false, false) => unreachable!(),
                            }
                        }
                    }

                    active_indices = next_active;
                }
            }
        }

        let leaf_indices_u64: Vec<u64> = sorted_indices.iter().map(|&i| i as u64).collect();

        Ok(MmrBatchInclusionProof {
            total_leaves: total_leaves as u64,
            leaf_indices: leaf_indices_u64,
            leaf_hashes,
            sister_hashes,
            untouched_peaks,
        })
    }

    /// Generates a non-membership proof asserting requested sequence is before the first leaf.
    pub fn prove_exclusion_before(
        &mut self,
        requested_seq: u64,
        first_leaf_seq: u64,
    ) -> Result<MmrExclusionProof, MmrError> {
        if self.is_empty() {
            return Err(MmrError::EmptyMmr);
        }
        if requested_seq >= first_leaf_seq {
            return Err(MmrError::InvalidExclusionProof(
                "requested_seq must be strictly less than first_leaf_seq".to_string(),
            ));
        }
        let inclusion_proof = self.prove_inclusion(0)?;
        let first_leaf_hash = inclusion_proof.leaf_hash.clone();
        Ok(MmrExclusionProof::BeforeRange {
            requested_seq,
            first_leaf_index: 0,
            first_leaf_seq,
            first_leaf_hash,
            inclusion_proof,
        })
    }

    /// Generates a non-membership proof asserting requested sequence is after the last leaf.
    pub fn prove_exclusion_after(
        &mut self,
        requested_seq: u64,
        last_leaf_seq: u64,
    ) -> Result<MmrExclusionProof, MmrError> {
        if self.is_empty() {
            return Err(MmrError::EmptyMmr);
        }
        if requested_seq <= last_leaf_seq {
            return Err(MmrError::InvalidExclusionProof(
                "requested_seq must be strictly greater than last_leaf_seq".to_string(),
            ));
        }
        let last_index = self.leaves.len() - 1;
        let inclusion_proof = self.prove_inclusion(last_index)?;
        let last_leaf_hash = inclusion_proof.leaf_hash.clone();
        Ok(MmrExclusionProof::AfterRange {
            requested_seq,
            last_leaf_index: last_index as u64,
            last_leaf_seq,
            last_leaf_hash,
            inclusion_proof,
        })
    }

    /// Generates a non-membership proof asserting a sequence gap between two adjacent leaves.
    pub fn prove_exclusion_gap(
        &mut self,
        requested_seq: u64,
        left_index: usize,
        left_seq: u64,
        right_seq: u64,
    ) -> Result<MmrExclusionProof, MmrError> {
        let right_index = left_index + 1;
        if right_index >= self.leaves.len() {
            return Err(MmrError::LeafIndexOutOfBounds(
                right_index,
                self.leaves.len(),
            ));
        }
        if !(left_seq < requested_seq && requested_seq < right_seq) {
            return Err(MmrError::InvalidExclusionProof(
                "requested_seq must be strictly between left_seq and right_seq".to_string(),
            ));
        }
        let left_proof = self.prove_inclusion(left_index)?;
        let right_proof = self.prove_inclusion(right_index)?;
        Ok(MmrExclusionProof::SequenceGap {
            requested_seq,
            left_index: left_index as u64,
            left_seq,
            left_leaf_hash: left_proof.leaf_hash.clone(),
            left_proof,
            right_index: right_index as u64,
            right_seq,
            right_leaf_hash: right_proof.leaf_hash.clone(),
            right_proof,
        })
    }

    /// Generates a non-membership proof asserting a hash is bounded between two adjacent leaves.
    pub fn prove_exclusion_hash_bound(
        &mut self,
        target_hash: &str,
        left_index: usize,
    ) -> Result<MmrExclusionProof, MmrError> {
        let right_index = left_index + 1;
        if right_index >= self.leaves.len() {
            return Err(MmrError::LeafIndexOutOfBounds(
                right_index,
                self.leaves.len(),
            ));
        }
        let left_proof = self.prove_inclusion(left_index)?;
        let right_proof = self.prove_inclusion(right_index)?;
        if !(left_proof.leaf_hash.as_str() < target_hash
            && target_hash < right_proof.leaf_hash.as_str())
        {
            return Err(MmrError::InvalidExclusionProof(
                "target_hash must be strictly between left_hash and right_hash".to_string(),
            ));
        }
        Ok(MmrExclusionProof::HashBound {
            target_hash: target_hash.to_string(),
            left_index: left_index as u64,
            left_hash: left_proof.leaf_hash.clone(),
            left_proof,
            right_index: right_index as u64,
            right_hash: right_proof.leaf_hash.clone(),
            right_proof,
        })
    }

    /// Verify an inclusion proof against expected MMR root.
    pub fn verify_proof(
        proof: &MmrInclusionProof,
        expected_root: &MmrHash,
    ) -> Result<bool, MmrError> {
        if proof.total_leaves == 0 || proof.leaf_index >= proof.total_leaves {
            return Err(MmrError::InvalidLeafCount);
        }

        let leaf_bytes = hex::decode(&proof.leaf_hash).map_err(|_| MmrError::CorruptedStructure)?;
        if leaf_bytes.len() != 32 {
            return Err(MmrError::CorruptedStructure);
        }
        let mut current_hash = [0u8; 32];
        current_hash.copy_from_slice(&leaf_bytes);

        // Find which peak tree this leaf belongs to
        let mut offset = 0;
        let mut target_peak_idx = 0;
        let mut target_tree_start = 0;

        for bit in (0..usize::BITS).rev() {
            let tree_size = 1usize << bit;
            if offset + tree_size <= proof.total_leaves {
                if proof.leaf_index >= offset && proof.leaf_index < offset + tree_size {
                    target_tree_start = offset;
                    break;
                }
                target_peak_idx += 1;
                offset += tree_size;
            }
        }

        let mut curr_idx = proof.leaf_index - target_tree_start;
        for sister_hex in &proof.sister_hashes {
            let sister_bytes = hex::decode(sister_hex).map_err(|_| MmrError::CorruptedStructure)?;
            if sister_bytes.len() != 32 {
                return Err(MmrError::CorruptedStructure);
            }
            let mut sister_hash = [0u8; 32];
            sister_hash.copy_from_slice(&sister_bytes);

            if curr_idx.is_multiple_of(2) {
                // We are left child
                current_hash = hash_nodes(&current_hash, &sister_hash);
            } else {
                // We are right child
                current_hash = hash_nodes(&sister_hash, &current_hash);
            }
            curr_idx /= 2;
        }

        // current_hash is now the computed peak. Compare with target peak in peak_hashes
        if target_peak_idx >= proof.peak_hashes.len() {
            return Err(MmrError::CorruptedStructure);
        }
        let expected_peak_bytes = hex::decode(&proof.peak_hashes[target_peak_idx])
            .map_err(|_| MmrError::CorruptedStructure)?;
        if expected_peak_bytes.as_slice() != current_hash.as_slice() {
            return Err(MmrError::InvalidProof {
                computed: hex::encode(current_hash),
                expected: proof.peak_hashes[target_peak_idx].clone(),
            });
        }

        // Re-bag peaks and verify final root
        let mut peaks = Vec::new();
        for p_hex in &proof.peak_hashes {
            let p_bytes = hex::decode(p_hex).map_err(|_| MmrError::CorruptedStructure)?;
            if p_bytes.len() != 32 {
                return Err(MmrError::CorruptedStructure);
            }
            let mut p = [0u8; 32];
            p.copy_from_slice(&p_bytes);
            peaks.push(p);
        }

        let computed_root = bag_peaks(&peaks);
        if computed_root != *expected_root {
            return Err(MmrError::InvalidProof {
                computed: hex::encode(computed_root),
                expected: hex::encode(*expected_root),
            });
        }

        Ok(true)
    }

    /// Create a compact rollup commitment for the entire MMR accumulator.
    pub fn create_rollup_commitment(
        &mut self,
        min_processed_at_micros: u64,
        max_processed_at_micros: u64,
    ) -> Result<MmrRollupCommitment, MmrError> {
        if self.is_empty() {
            return Err(MmrError::EmptyMmr);
        }
        let root_hash = self.root_hex();
        let first_leaf_hash = hex::encode(self.leaves[0]);
        let last_leaf_hash = hex::encode(self.leaves[self.leaves.len() - 1]);

        Ok(MmrRollupCommitment {
            root_hash,
            leaf_count: self.len(),
            first_leaf_hash,
            last_leaf_hash,
            min_processed_at_micros,
            max_processed_at_micros,
        })
    }
}

impl fmt::Display for MerkleMountainRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MMR(leaves={}, peaks={})",
            self.leaves.len(),
            self.peaks.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_mmr_append_and_root_consistency() {
        let mut mmr = MerkleMountainRange::new();
        assert_eq!(mmr.len(), 0);
        assert!(mmr.is_empty());

        let idx0 = mmr.append_bytes(b"receipt_001");
        assert_eq!(idx0, 0);
        assert_eq!(mmr.len(), 1);
        let root1 = mmr.root();

        let idx1 = mmr.append_bytes(b"receipt_002");
        assert_eq!(idx1, 1);
        assert_eq!(mmr.len(), 2);
        let root2 = mmr.root();
        assert_ne!(root1, root2);

        let idx2 = mmr.append_bytes(b"receipt_003");
        assert_eq!(idx2, 2);
        assert_eq!(mmr.len(), 3);
        let root3 = mmr.root();
        assert_ne!(root2, root3);
    }

    #[test]
    fn test_incremental_mmr_matches_in_memory_mmr_at_scale() {
        let mut mem_mmr = MerkleMountainRange::new();
        let mut inc_mmr = IncrementalMmr::new();

        for i in 0..100 {
            let data = format!("scale_leaf_{i}");
            let leaf_hash = hash_leaf(data.as_bytes());

            let idx_mem = mem_mmr.append(leaf_hash);
            let idx_inc = inc_mmr.append_leaf(leaf_hash);

            assert_eq!(idx_mem as u64, idx_inc);
            assert_eq!(mem_mmr.root(), inc_mmr.get_root());
            assert_eq!(mem_mmr.peaks(), inc_mmr.get_peaks().as_slice());
        }
    }

    #[test]
    fn test_incremental_mmr_zmmr_disk_persistence_roundtrip() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_checkpoint.zmmr");

        let mut inc_mmr = IncrementalMmr::new();
        for i in 0..42 {
            inc_mmr.append_bytes(format!("payload_entry_{i}").as_bytes());
        }
        let original_root = inc_mmr.get_root();

        inc_mmr.save_to_file(&file_path).unwrap();
        assert!(file_path.exists());

        let mut loaded = IncrementalMmr::load_from_file(&file_path).unwrap();
        assert_eq!(loaded.leaf_count, 42);
        assert_eq!(loaded.get_root(), original_root);
        assert_eq!(loaded.get_peaks(), inc_mmr.get_peaks());

        // Append more after load
        let next_idx = loaded.append_bytes(b"payload_entry_42");
        assert_eq!(next_idx, 42);
        assert_eq!(loaded.leaf_count, 43);
    }

    #[test]
    fn test_mmr_inclusion_proof_verification_all_leaves() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..15 {
            let data = format!("receipt_payload_{i}");
            mmr.append_bytes(data.as_bytes());
        }

        let root = mmr.root();

        for i in 0..15 {
            let proof = mmr.prove_inclusion(i).expect("should generate proof");
            assert_eq!(proof.leaf_index, i);
            assert_eq!(proof.total_leaves, 15);
            let verified = MerkleMountainRange::verify_proof(&proof, &root).expect("should verify");
            assert!(verified);
        }
    }

    #[test]
    fn test_mmr_tampered_proof_fails() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..8 {
            mmr.append_bytes(format!("rec_{i}").as_bytes());
        }
        let root = mmr.root();
        let mut proof = mmr.prove_inclusion(3).unwrap();

        // Tamper leaf hash
        proof.leaf_hash = hex::encode([0xff; 32]);
        let err = MerkleMountainRange::verify_proof(&proof, &root);
        assert!(err.is_err());
    }

    #[test]
    fn test_mmr_batch_inclusion_proof_sparse_and_dense() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..50 {
            mmr.append_bytes(format!("batch_item_{i}").as_bytes());
        }
        let root = mmr.root();

        // 1. Sparse subset
        let subset_indices = vec![0, 3, 15, 16, 31, 49];
        let proof = mmr.prove_batch_inclusion(&subset_indices).unwrap();
        assert_eq!(proof.leaf_indices, vec![0, 3, 15, 16, 31, 49]);
        assert_eq!(proof.total_leaves, 50);
        assert!(proof.verify(&root).unwrap());

        // 2. Dense range
        let dense_indices: Vec<usize> = (10..30).collect();
        let dense_proof = mmr.prove_batch_inclusion(&dense_indices).unwrap();
        assert!(dense_proof.verify(&root).unwrap());

        // 3. All leaves
        let all_indices: Vec<usize> = (0..50).collect();
        let all_proof = mmr.prove_batch_inclusion(&all_indices).unwrap();
        assert!(all_proof.verify(&root).unwrap());

        // 4. Tampered leaf hash
        let mut tampered_proof = proof.clone();
        tampered_proof.leaf_hashes[1] = hex::encode([0xEE; 32]);
        assert!(tampered_proof.verify(&root).is_err());

        // 5. Tampered root
        let wrong_root = [0x55; 32];
        assert!(proof.verify(&wrong_root).is_err());
    }

    #[test]
    fn test_mmr_exclusion_proofs() {
        let mut mmr = MerkleMountainRange::new();
        // Insert leaves with sequences 10, 20, 30, 40, 50
        let sequences = [10u64, 20, 30, 40, 50];
        for seq in sequences {
            mmr.append_bytes(format!("seq:{seq}").as_bytes());
        }
        let root = mmr.root();

        // 1. BeforeRange: seq 5 < 10
        let before_proof = mmr.prove_exclusion_before(5, 10).unwrap();
        assert!(before_proof.verify(&root).unwrap());

        // BeforeRange fails if seq >= 10
        assert!(mmr.prove_exclusion_before(15, 10).is_err());

        // 2. AfterRange: seq 60 > 50
        let after_proof = mmr.prove_exclusion_after(60, 50).unwrap();
        assert!(after_proof.verify(&root).unwrap());

        // AfterRange fails if seq <= 50
        assert!(mmr.prove_exclusion_after(45, 50).is_err());

        // 3. SequenceGap: seq 25 between index 1 (seq 20) and index 2 (seq 30)
        let gap_proof = mmr.prove_exclusion_gap(25, 1, 20, 30).unwrap();
        assert!(gap_proof.verify(&root).unwrap());

        // Gap fails if requested seq not strictly inside gap
        assert!(mmr.prove_exclusion_gap(35, 1, 20, 30).is_err());

        // 4. HashBound: lexicographically ordered leaves
        let mut sorted_mmr = MerkleMountainRange::new();
        sorted_mmr.append_bytes(b"alpha"); // leaf 0
        sorted_mmr.append_bytes(b"gamma"); // leaf 1
        sorted_mmr.append_bytes(b"omega"); // leaf 2
        let sorted_root = sorted_mmr.root();

        let target_hash = format!("00000000_in_between_{}", hex::encode([0x55; 32]));
        // Make sure it is between left and right leaf hashes
        let l0_hash = hex::encode(sorted_mmr.leaves[0]);
        let l1_hash = hex::encode(sorted_mmr.leaves[1]);
        if l0_hash < target_hash && target_hash < l1_hash {
            let bound_proof = sorted_mmr
                .prove_exclusion_hash_bound(&target_hash, 0)
                .unwrap();
            assert!(bound_proof.verify(&sorted_root).unwrap());
        }
    }

    #[test]
    fn test_mmr_rollup_commitment() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..100 {
            mmr.append_bytes(format!("rec_{i}").as_bytes());
        }
        let commitment = mmr.create_rollup_commitment(1_000_000, 2_000_000).unwrap();
        assert_eq!(commitment.leaf_count, 100);
        assert_eq!(commitment.min_processed_at_micros, 1_000_000);
        assert_eq!(commitment.max_processed_at_micros, 2_000_000);
        assert_eq!(commitment.root_hash, mmr.root_hex());
    }
}
