//! WebSocket Transport Bridge (RFC 6455) for AI Agent messaging.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{Result, RivunGatewayError};

pub const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
pub const MAX_WS_FRAME_SIZE: usize = 4 * 1024 * 1024; // 4MB

pub const WS_OPCODE_CONTINUATION: u8 = 0x0;
pub const WS_OPCODE_TEXT: u8 = 0x1;
pub const WS_OPCODE_BINARY: u8 = 0x2;
pub const WS_OPCODE_CLOSE: u8 = 0x8;
pub const WS_OPCODE_PING: u8 = 0x9;
pub const WS_OPCODE_PONG: u8 = 0xA;

pub const WS_CLOSE_NORMAL: u16 = 1000;
pub const WS_CLOSE_MESSAGE_TOO_BIG: u16 = 1009;

/// Compute Sec-WebSocket-Accept from Sec-WebSocket-Key
pub fn compute_ws_accept(key: &str) -> String {
    let combined = format!("{}{}", key.trim(), WS_GUID);
    let digest = sha1_digest(combined.as_bytes());
    STANDARD.encode(digest)
}

/// Self-contained standard SHA-1 (RFC 3174)
pub fn sha1_digest(data: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    let msg_len = data.len();
    let bit_len = (msg_len as u64) * 8;

    let mut padded = Vec::from(data);
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (i, &w_i) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w_i);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsFrame {
    pub fin: bool,
    pub opcode: u8,
    pub payload: Vec<u8>,
}

impl WsFrame {
    pub fn text(data: impl Into<String>) -> Self {
        Self {
            fin: true,
            opcode: WS_OPCODE_TEXT,
            payload: data.into().into_bytes(),
        }
    }

    pub fn binary(data: Vec<u8>) -> Self {
        Self {
            fin: true,
            opcode: WS_OPCODE_BINARY,
            payload: data,
        }
    }

    pub fn close(code: u16, reason: &str) -> Self {
        let mut payload = Vec::with_capacity(2 + reason.len());
        payload.extend_from_slice(&code.to_be_bytes());
        payload.extend_from_slice(reason.as_bytes());
        Self {
            fin: true,
            opcode: WS_OPCODE_CLOSE,
            payload,
        }
    }

    pub fn ping(data: Vec<u8>) -> Self {
        Self {
            fin: true,
            opcode: WS_OPCODE_PING,
            payload: data,
        }
    }

    pub fn pong(data: Vec<u8>) -> Self {
        Self {
            fin: true,
            opcode: WS_OPCODE_PONG,
            payload: data,
        }
    }
}

pub struct WebSocketHandler {
    max_frame_size: usize,
}

impl WebSocketHandler {
    pub fn new(max_frame_size: usize) -> Self {
        Self { max_frame_size }
    }

    pub fn default_max() -> Self {
        Self::new(MAX_WS_FRAME_SIZE)
    }

    pub async fn read_frame<R: AsyncRead + Unpin>(&self, reader: &mut R) -> Result<WsFrame> {
        let mut header = [0u8; 2];
        reader.read_exact(&mut header).await?;

        let fin = (header[0] & 0x80) != 0;
        let opcode = header[0] & 0x0F;
        let masked = (header[1] & 0x80) != 0;
        let mut payload_len = (header[1] & 0x7F) as usize;

        if payload_len == 126 {
            let mut ext = [0u8; 2];
            reader.read_exact(&mut ext).await?;
            payload_len = u16::from_be_bytes(ext) as usize;
        } else if payload_len == 127 {
            let mut ext = [0u8; 8];
            reader.read_exact(&mut ext).await?;
            let len64 = u64::from_be_bytes(ext);
            if len64 > self.max_frame_size as u64 {
                return Err(RivunGatewayError::FrameSizeExceeded {
                    size: len64 as usize,
                    max: self.max_frame_size,
                });
            }
            payload_len = len64 as usize;
        }

        if payload_len > self.max_frame_size {
            return Err(RivunGatewayError::FrameSizeExceeded {
                size: payload_len,
                max: self.max_frame_size,
            });
        }

        let masking_key = if masked {
            let mut mask = [0u8; 4];
            reader.read_exact(&mut mask).await?;
            Some(mask)
        } else {
            None
        };

        let mut payload = vec![0u8; payload_len];
        if payload_len > 0 {
            reader.read_exact(&mut payload).await?;
        }

        if let Some(mask) = masking_key {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[i % 4];
            }
        }

        Ok(WsFrame {
            fin,
            opcode,
            payload,
        })
    }

    pub async fn write_frame<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        frame: &WsFrame,
    ) -> Result<()> {
        let payload_len = frame.payload.len();
        if payload_len > self.max_frame_size {
            return Err(RivunGatewayError::FrameSizeExceeded {
                size: payload_len,
                max: self.max_frame_size,
            });
        }

        let mut header = Vec::with_capacity(10);
        let b0 = if frame.fin { 0x80 } else { 0x00 } | (frame.opcode & 0x0F);
        header.push(b0);

        if payload_len < 126 {
            header.push(payload_len as u8);
        } else if payload_len <= 0xFFFF {
            header.push(126);
            header.extend_from_slice(&(payload_len as u16).to_be_bytes());
        } else {
            header.push(127);
            header.extend_from_slice(&(payload_len as u64).to_be_bytes());
        }

        writer.write_all(&header).await?;
        if payload_len > 0 {
            writer.write_all(&frame.payload).await?;
        }
        writer.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc6455_accept_calculation() {
        // RFC 6455 Section 1.3 example
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept = compute_ws_accept(key);
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }
}
