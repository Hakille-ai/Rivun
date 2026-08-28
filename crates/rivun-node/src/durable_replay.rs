use anyhow::{Result, bail};
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use rivun_core::RivunFrame;

pub const DURABLE_FRAME_MAGIC: &[u8; 8] = b"ZAPFRM01";
pub const DURABLE_FRAME_RECORD_LEN: usize = 8 + 16 + 16; // timestamp_micros (8B BE) + source_node (16B) + fingerprint (16B)

#[derive(Debug)]
pub struct DurableReplayStore {
    path: PathBuf,
    capacity: usize,
    max_clock_skew_micros: u64,
    seen: HashMap<[u8; 16], (u64, Uuid)>,
    order: VecDeque<([u8; 16], Uuid, u64)>,
    file: Option<File>,
}

impl DurableReplayStore {
    pub fn open(
        path: impl AsRef<Path>,
        capacity: usize,
        max_clock_skew_micros: u64,
    ) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut seen = HashMap::new();
        let mut order = VecDeque::new();
        let now_micros = rivun_core::now_micros().unwrap_or_default();

        if path.exists() {
            let mut file = OpenOptions::new().read(true).write(true).open(&path)?;
            let mut magic = [0_u8; 8];
            if file.read_exact(&mut magic).is_ok() && &magic == DURABLE_FRAME_MAGIC {
                let mut buf = [0_u8; DURABLE_FRAME_RECORD_LEN];
                let mut valid_records = 0_usize;
                while file.read_exact(&mut buf).is_ok() {
                    let timestamp_micros = u64::from_be_bytes(buf[0..8].try_into().unwrap());
                    let source_node = Uuid::from_bytes(buf[8..24].try_into().unwrap());
                    let mut fingerprint = [0_u8; 16];
                    fingerprint.copy_from_slice(&buf[24..40]);
                    if max_clock_skew_micros == 0
                        || now_micros.saturating_sub(timestamp_micros) <= max_clock_skew_micros
                    {
                        seen.insert(fingerprint, (timestamp_micros, source_node));
                        order.push_back((fingerprint, source_node, timestamp_micros));
                    }
                    valid_records += 1;
                }
                let valid_len = 8 + (valid_records * DURABLE_FRAME_RECORD_LEN) as u64;
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
            file.write_all(DURABLE_FRAME_MAGIC)?;
            file.flush()?;
        }

        Ok(Self {
            path,
            capacity,
            max_clock_skew_micros,
            seen,
            order,
            file: Some(file),
        })
    }

    pub fn check_and_insert(&mut self, frame: &RivunFrame, now_micros: u64) -> Result<()> {
        if self.capacity == 0 {
            return Ok(());
        }

        let ts = frame.header.timestamp_micros;
        if self.max_clock_skew_micros > 0
            && (ts.saturating_add(self.max_clock_skew_micros) < now_micros
                || ts > now_micros.saturating_add(self.max_clock_skew_micros))
        {
            bail!("frame timestamp outside clock skew window");
        }

        let fingerprint = frame_fingerprint(frame);
        if let Some((prev_ts, source)) = self.seen.get(&fingerprint) {
            bail!(
                "replayed frame rejected: source_node={}, timestamp_micros={}, signature_hint={}",
                source,
                prev_ts,
                hex_hint(&frame.header.rivun_sign)
            );
        }

        if let Some(file) = &mut self.file {
            let mut buf = [0_u8; DURABLE_FRAME_RECORD_LEN];
            buf[0..8].copy_from_slice(&ts.to_be_bytes());
            buf[8..24].copy_from_slice(frame.header.source_node.as_bytes());
            buf[24..40].copy_from_slice(&fingerprint);
            file.write_all(&buf)?;
            file.sync_data()?;
        }

        self.seen
            .insert(fingerprint, (ts, frame.header.source_node));
        self.order
            .push_back((fingerprint, frame.header.source_node, ts));

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
            tmp_file.write_all(DURABLE_FRAME_MAGIC)?;
            for (fingerprint, source_node, timestamp_micros) in &self.order {
                if self.max_clock_skew_micros == 0
                    || now_micros.saturating_sub(*timestamp_micros) <= self.max_clock_skew_micros
                {
                    let mut buf = [0_u8; DURABLE_FRAME_RECORD_LEN];
                    buf[0..8].copy_from_slice(&timestamp_micros.to_be_bytes());
                    buf[8..24].copy_from_slice(source_node.as_bytes());
                    buf[24..40].copy_from_slice(fingerprint);
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

pub fn frame_fingerprint(frame: &RivunFrame) -> [u8; 16] {
    let hash = blake3::hash(&frame.encode());
    hash.as_bytes()[..16].try_into().unwrap()
}

fn hex_hint(hint: &[u8]) -> String {
    hint.iter()
        .take(4)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use rivun_core::{RivunFlags, RivunFrame};

    #[test]
    fn durable_replay_store_persists_fingerprints_across_restart() {
        let dir = tempdir().unwrap();
        let wal_path = dir.path().join("frames.wal");
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let now = rivun_core::now_micros().unwrap();

        let frame = RivunFrame::with_timestamp(
            source,
            target,
            RivunFlags::ENCRYPTED,
            now,
            bytes::Bytes::from_static(b"frame1"),
        )
        .unwrap();

        // 1st run
        {
            let mut store = DurableReplayStore::open(&wal_path, 100, 300_000_000).unwrap();
            store.check_and_insert(&frame, now).unwrap();
            assert!(store.check_and_insert(&frame, now).is_err());
        }

        // Re-open store (node restart)
        {
            let mut store = DurableReplayStore::open(&wal_path, 100, 300_000_000).unwrap();
            let err = store.check_and_insert(&frame, now).unwrap_err();
            assert!(err.to_string().contains("replayed frame rejected"));
        }
    }
}
