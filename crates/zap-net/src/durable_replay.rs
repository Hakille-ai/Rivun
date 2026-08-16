use crate::{NONCE_LEN, Result, ZapNetError};
use std::collections::{HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const DURABLE_NONCE_MAGIC: &[u8; 8] = b"ZAPNONC1";
pub const DURABLE_NONCE_RECORD_LEN: usize = 8 + 16 + 12; // timestamp_micros (u64 BE) + node_id (16B) + nonce (12B)

#[derive(Debug)]
pub struct DurableNonceRecord {
    pub timestamp_micros: u64,
    pub node_id: Uuid,
    pub nonce: [u8; NONCE_LEN],
}

#[derive(Debug)]
pub struct DurableNonceStore {
    path: PathBuf,
    capacity: usize,
    max_age_micros: u64,
    seen: HashSet<[u8; NONCE_LEN]>,
    order: VecDeque<([u8; NONCE_LEN], Uuid, u64)>,
    file: Option<File>,
}

impl DurableNonceStore {
    pub fn open(path: impl AsRef<Path>, capacity: usize, max_age_micros: u64) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut seen = HashSet::new();
        let mut order = VecDeque::new();
        let now_micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        if path.exists() {
            let mut file = OpenOptions::new().read(true).write(true).open(&path)?;
            let mut magic = [0_u8; 8];
            if file.read_exact(&mut magic).is_ok() && &magic == DURABLE_NONCE_MAGIC {
                let mut buf = [0_u8; DURABLE_NONCE_RECORD_LEN];
                let mut valid_records = 0_usize;
                while file.read_exact(&mut buf).is_ok() {
                    let timestamp_micros = u64::from_be_bytes(buf[0..8].try_into().unwrap());
                    let node_id = Uuid::from_slice(&buf[8..24]).unwrap_or_default();
                    if max_age_micros == 0
                        || now_micros.saturating_sub(timestamp_micros) <= max_age_micros
                    {
                        let mut nonce = [0_u8; NONCE_LEN];
                        nonce.copy_from_slice(&buf[24..36]);
                        seen.insert(nonce);
                        order.push_back((nonce, node_id, timestamp_micros));
                    }
                    valid_records += 1;
                }
                let valid_len = 8 + (valid_records * DURABLE_NONCE_RECORD_LEN) as u64;
                if file.metadata()?.len() > valid_len {
                    file.set_len(valid_len)?;
                    file.flush()?;
                }
            }
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;

        if file.metadata()?.len() == 0 {
            file.write_all(DURABLE_NONCE_MAGIC)?;
            file.flush()?;
        }

        Ok(Self {
            path,
            capacity,
            max_age_micros,
            seen,
            order,
            file: Some(file),
        })
    }

    pub fn contains(&self, nonce: &[u8; NONCE_LEN]) -> bool {
        self.seen.contains(nonce)
    }

    pub fn remember(
        &mut self,
        node_id: Uuid,
        nonce: [u8; NONCE_LEN],
        timestamp_micros: u64,
    ) -> Result<()> {
        if self.capacity == 0 {
            return Ok(());
        }
        if self.seen.contains(&nonce) {
            return Err(ZapNetError::ReplayedDatagramNonce { node_id });
        }

        if let Some(file) = &mut self.file {
            let mut buf = [0_u8; DURABLE_NONCE_RECORD_LEN];
            buf[0..8].copy_from_slice(&timestamp_micros.to_be_bytes());
            buf[8..24].copy_from_slice(node_id.as_bytes());
            buf[24..36].copy_from_slice(&nonce);
            file.write_all(&buf)?;
            file.sync_data()?;
        }

        self.seen.insert(nonce);
        self.order.push_back((nonce, node_id, timestamp_micros));

        while self.order.len() > self.capacity {
            if let Some((expired, _, _)) = self.order.pop_front() {
                self.seen.remove(&expired);
            }
        }
        Ok(())
    }

    pub fn compact(&mut self, now_micros: u64) -> Result<()> {
        let tmp_path = self.path.with_extension("tmp");
        {
            let mut tmp_file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&tmp_path)?;
            tmp_file.write_all(DURABLE_NONCE_MAGIC)?;
            for (nonce, node_id, timestamp_micros) in &self.order {
                if self.max_age_micros == 0
                    || now_micros.saturating_sub(*timestamp_micros) <= self.max_age_micros
                {
                    let mut buf = [0_u8; DURABLE_NONCE_RECORD_LEN];
                    buf[0..8].copy_from_slice(&timestamp_micros.to_be_bytes());
                    buf[8..24].copy_from_slice(node_id.as_bytes());
                    buf[24..36].copy_from_slice(nonce);
                    tmp_file.write_all(&buf)?;
                }
            }
            tmp_file.sync_all()?;
        }
        std::fs::rename(&tmp_path, &self.path)?;
        self.file = Some(
            OpenOptions::new()
                .append(true)
                .read(true)
                .open(&self.path)?,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn durable_nonce_store_persists_nonces_across_restarts() {
        let dir = tempdir().unwrap();
        let wal_path = dir.path().join("nonces.wal");
        let node_id = Uuid::new_v4();
        let nonce = [0x42_u8; NONCE_LEN];
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        {
            let mut store = DurableNonceStore::open(&wal_path, 100, 3_600_000_000).unwrap();
            store.remember(node_id, nonce, now).unwrap();
            assert!(store.contains(&nonce));
            let err = store.remember(node_id, nonce, now).unwrap_err();
            assert!(matches!(err, ZapNetError::ReplayedDatagramNonce { .. }));
        }

        // Re-open store from same WAL (simulating node restart)
        {
            let mut store = DurableNonceStore::open(&wal_path, 100, 3_600_000_000).unwrap();
            assert!(store.contains(&nonce));
            let err = store.remember(node_id, nonce, now).unwrap_err();
            assert!(matches!(err, ZapNetError::ReplayedDatagramNonce { .. }));
        }
    }
}
