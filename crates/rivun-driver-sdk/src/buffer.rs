//! Zero-copy buffer views, pinned memory management, and memory slice utilities.

use crate::error::BufferError;
use std::ops::{Deref, DerefMut};

/// Contiguous memory buffer pinned at a stable address for zero-copy I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedBuffer {
    ptr: u32,
    capacity: usize,
    len: usize,
    data: Vec<u8>,
}

impl PinnedBuffer {
    /// Create a new pinned buffer with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let mut data = vec![0_u8; capacity];
        let ptr = data.as_mut_ptr() as usize as u32;
        Self {
            ptr,
            capacity,
            len: 0,
            data,
        }
    }

    /// Construct a pinned buffer from an existing memory vector.
    pub fn from_vec(data: Vec<u8>) -> Self {
        let capacity = data.capacity();
        let len = data.len();
        let ptr = data.as_ptr() as usize as u32;
        Self {
            ptr,
            capacity,
            len,
            data,
        }
    }

    pub fn ptr(&self) -> u32 {
        self.ptr
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data[..self.len]
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn set_len(&mut self, new_len: usize) -> Result<(), BufferError> {
        if new_len > self.capacity {
            return Err(BufferError::CapacityExceeded {
                requested: new_len,
                capacity: self.capacity,
            });
        }
        self.len = new_len;
        Ok(())
    }

    /// Write bytes into the pinned buffer starting from current `len`.
    pub fn write(&mut self, bytes: &[u8]) -> Result<usize, BufferError> {
        let remaining = self.capacity.saturating_sub(self.len);
        if bytes.len() > remaining {
            return Err(BufferError::CapacityExceeded {
                requested: self.len.saturating_add(bytes.len()),
                capacity: self.capacity,
            });
        }
        self.data[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(bytes.len())
    }

    /// Appends the contents of a byte slice to the buffer.
    pub fn extend_from_slice(&mut self, bytes: &[u8]) -> Result<(), BufferError> {
        self.write(bytes).map(|_| ())
    }

    /// Borrow an immutable sub-slice view.
    pub fn slice(&self, offset: usize, len: usize) -> Result<BufferSlice<'_>, BufferError> {
        let end = offset.checked_add(len).ok_or(BufferError::OutOfBounds {
            offset,
            len,
            bound: self.len,
        })?;
        if end > self.len {
            return Err(BufferError::OutOfBounds {
                offset,
                len,
                bound: self.len,
            });
        }
        Ok(BufferSlice::new(&self.data[offset..end]))
    }

    /// Borrow a mutable sub-slice view.
    pub fn slice_mut(
        &mut self,
        offset: usize,
        len: usize,
    ) -> Result<BufferSliceMut<'_>, BufferError> {
        let end = offset.checked_add(len).ok_or(BufferError::OutOfBounds {
            offset,
            len,
            bound: self.capacity,
        })?;
        if end > self.capacity {
            return Err(BufferError::OutOfBounds {
                offset,
                len,
                bound: self.capacity,
            });
        }
        if end > self.len {
            self.len = end;
        }
        Ok(BufferSliceMut::new(&mut self.data[offset..end]))
    }

    /// Consumes the buffer and returns the underlying `Vec<u8>`.
    pub fn into_vec(mut self) -> Vec<u8> {
        self.data.truncate(self.len);
        self.data
    }
}

/// Immutable zero-copy slice over a memory region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferSlice<'a> {
    data: &'a [u8],
}

impl<'a> BufferSlice<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn subslice(&self, offset: usize, len: usize) -> Result<Self, BufferError> {
        let end = offset.checked_add(len).ok_or(BufferError::OutOfBounds {
            offset,
            len,
            bound: self.data.len(),
        })?;
        if end > self.data.len() {
            return Err(BufferError::OutOfBounds {
                offset,
                len,
                bound: self.data.len(),
            });
        }
        Ok(Self::new(&self.data[offset..end]))
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.data.to_vec()
    }
}

impl<'a> Deref for BufferSlice<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a> AsRef<[u8]> for BufferSlice<'a> {
    fn as_ref(&self) -> &[u8] {
        self.data
    }
}

/// Mutable zero-copy slice over a memory region.
#[derive(Debug)]
pub struct BufferSliceMut<'a> {
    data: &'a mut [u8],
}

impl<'a> BufferSliceMut<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self { data }
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        self.data
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn write_slice(&mut self, src: &[u8]) -> Result<usize, BufferError> {
        if src.len() > self.data.len() {
            return Err(BufferError::CapacityExceeded {
                requested: src.len(),
                capacity: self.data.len(),
            });
        }
        self.data[..src.len()].copy_from_slice(src);
        Ok(src.len())
    }

    pub fn split_at_mut(
        self,
        mid: usize,
    ) -> Result<(BufferSliceMut<'a>, BufferSliceMut<'a>), BufferError> {
        if mid > self.data.len() {
            return Err(BufferError::OutOfBounds {
                offset: mid,
                len: 0,
                bound: self.data.len(),
            });
        }
        let (first, second) = self.data.split_at_mut(mid);
        Ok((BufferSliceMut::new(first), BufferSliceMut::new(second)))
    }
}

impl<'a> Deref for BufferSliceMut<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a> DerefMut for BufferSliceMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<'a> AsRef<[u8]> for BufferSliceMut<'a> {
    fn as_ref(&self) -> &[u8] {
        self.data
    }
}

impl<'a> AsMut<[u8]> for BufferSliceMut<'a> {
    fn as_mut(&mut self) -> &mut [u8] {
        self.data
    }
}

/// Guest-Host memory pointer translation utilities.
pub struct MemoryMapper;

impl MemoryMapper {
    /// Validates that a guest pointer and length fit entirely inside memory bounds.
    pub fn validate_range(
        guest_ptr: u32,
        len: usize,
        total_mem_size: usize,
    ) -> Result<(), BufferError> {
        let start = guest_ptr as usize;
        let end = start.checked_add(len).ok_or(BufferError::InvalidPointer {
            ptr: guest_ptr,
            len: len as u32,
        })?;
        if end > total_mem_size {
            return Err(BufferError::OutOfBounds {
                offset: start,
                len,
                bound: total_mem_size,
            });
        }
        Ok(())
    }

    /// Safely translates a guest pointer into an immutable byte slice.
    pub fn translate_slice(
        memory: &[u8],
        guest_ptr: u32,
        len: usize,
    ) -> Result<&[u8], BufferError> {
        Self::validate_range(guest_ptr, len, memory.len())?;
        let start = guest_ptr as usize;
        Ok(&memory[start..start + len])
    }

    /// Safely translates a guest pointer into a mutable byte slice.
    pub fn translate_slice_mut(
        memory: &mut [u8],
        guest_ptr: u32,
        len: usize,
    ) -> Result<&mut [u8], BufferError> {
        Self::validate_range(guest_ptr, len, memory.len())?;
        let start = guest_ptr as usize;
        Ok(&mut memory[start..start + len])
    }
}

/// Zero-copy immutable view over linear guest/host memory with Blake3 hashing support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroCopyBuffer<'a> {
    data: &'a [u8],
}

impl<'a> ZeroCopyBuffer<'a> {
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn as_slice(&self) -> &'a [u8] {
        self.data
    }

    pub fn slice(&self, offset: usize, len: usize) -> Result<Self, BufferError> {
        let end = offset.checked_add(len).ok_or(BufferError::OutOfBounds {
            offset,
            len,
            bound: self.data.len(),
        })?;
        if end > self.data.len() {
            return Err(BufferError::OutOfBounds {
                offset,
                len,
                bound: self.data.len(),
            });
        }
        Ok(Self::new(&self.data[offset..end]))
    }

    /// Compute cryptographic Blake3 hash of the zero-copy buffer.
    pub fn hash(&self) -> [u8; 32] {
        *blake3::hash(self.data).as_bytes()
    }

    /// Compute hex-encoded Blake3 digest.
    pub fn hex_digest(&self) -> String {
        blake3::hash(self.data).to_hex().to_string()
    }
}

impl<'a> Deref for ZeroCopyBuffer<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a> AsRef<[u8]> for ZeroCopyBuffer<'a> {
    fn as_ref(&self) -> &[u8] {
        self.data
    }
}

/// Zero-copy IPC buffer view binding an IPC message envelope to a borrowed payload slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcBufferView<'a> {
    pub channel_id: u32,
    pub sequence: u64,
    pub timestamp_micros: u64,
    pub flags: u32,
    pub payload: BufferSlice<'a>,
}

impl<'a> IpcBufferView<'a> {
    pub fn new(
        channel_id: u32,
        sequence: u64,
        timestamp_micros: u64,
        flags: u32,
        payload: &'a [u8],
    ) -> Self {
        Self {
            channel_id,
            sequence,
            timestamp_micros,
            flags,
            payload: BufferSlice::new(payload),
        }
    }

    pub fn channel_id(&self) -> u32 {
        self.channel_id
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn timestamp_micros(&self) -> u64 {
        self.timestamp_micros
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn payload(&self) -> &[u8] {
        self.payload.as_bytes()
    }

    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }

    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }

    /// Calculate Blake3 hash of the payload.
    pub fn payload_hash(&self) -> [u8; 32] {
        *blake3::hash(self.payload.as_bytes()).as_bytes()
    }

    /// Subslice the underlying payload view.
    pub fn subslice(&self, offset: usize, len: usize) -> Result<Self, BufferError> {
        let sub = self.payload.subslice(offset, len)?;
        Ok(Self {
            channel_id: self.channel_id,
            sequence: self.sequence,
            timestamp_micros: self.timestamp_micros,
            flags: self.flags,
            payload: sub,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinned_buffer_write_and_slice() {
        let mut buf = PinnedBuffer::with_capacity(1024);
        assert_eq!(buf.capacity(), 1024);
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());

        let bytes = b"hello rivun zero-copy buffer";
        let written = buf.write(bytes).unwrap();
        assert_eq!(written, bytes.len());
        assert_eq!(buf.len(), bytes.len());
        assert_eq!(buf.as_slice(), bytes);

        let slice = buf.slice(6, 5).unwrap();
        assert_eq!(slice.as_bytes(), b"rivun");

        let subslice = slice.subslice(0, 2).unwrap();
        assert_eq!(subslice.as_bytes(), b"ri");
    }

    #[test]
    fn test_pinned_buffer_overflow() {
        let mut buf = PinnedBuffer::with_capacity(10);
        let err = buf.write(b"0123456789012345").unwrap_err();
        assert_eq!(
            err,
            BufferError::CapacityExceeded {
                requested: 16,
                capacity: 10
            }
        );
    }

    #[test]
    fn test_memory_mapper() {
        let memory = vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let slice = MemoryMapper::translate_slice(&memory, 2, 3).unwrap();
        assert_eq!(slice, &[0xCC, 0xDD, 0xEE]);

        let err = MemoryMapper::translate_slice(&memory, 4, 4).unwrap_err();
        assert_eq!(
            err,
            BufferError::OutOfBounds {
                offset: 4,
                len: 4,
                bound: 6
            }
        );
    }

    #[test]
    fn test_zero_copy_buffer_and_hashing() {
        let data = b"sensor_telemetry_frame_payload_012345";
        let zcb = ZeroCopyBuffer::new(data);
        assert_eq!(zcb.len(), data.len());
        assert_eq!(&zcb[..6], b"sensor");

        let sub = zcb.slice(7, 9).unwrap();
        assert_eq!(sub.as_slice(), b"telemetry");

        let hash1 = zcb.hash();
        let hash2 = blake3::hash(data);
        assert_eq!(hash1, *hash2.as_bytes());
        assert_eq!(zcb.hex_digest(), hash2.to_hex().to_string());
    }

    #[test]
    fn test_ipc_buffer_view_slicing() {
        let payload = b"ipc_packet_header_and_body";
        let view = IpcBufferView::new(42, 100, 1_700_000_000_000, 0x01, payload);
        assert_eq!(view.channel_id(), 42);
        assert_eq!(view.sequence(), 100);
        assert_eq!(view.flags(), 0x01);
        assert_eq!(view.payload(), payload);

        let subview = view.subslice(0, 10).unwrap();
        assert_eq!(subview.payload(), b"ipc_packet");
        assert_eq!(subview.sequence(), 100);
    }
}
