use crate::error::{AudioModemError, Result};
use crate::{FRAME_HEADER_SIZE, MAX_PAYLOAD_SIZE};

/// CRC-16-CCITT for payload integrity verification
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u32 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u32) << 8;
        for _ in 0..8 {
            crc <<= 1;
            if crc & 0x10000 != 0 {
                crc ^= 0x1021;
            }
        }
    }
    (crc & 0xFFFF) as u16
}

/// Proper CRC-8 using polynomial 0xD5 (255 = x^8 + x^7 + x^6 + x^4 + x^2 + 1)
/// This is a standard polynomial with excellent error detection properties
/// Detects all single-bit errors, many multi-bit patterns, and burst errors up to 7 bits
fn crc8(data: &[u8]) -> u8 {
    const POLYNOMIAL: u8 = 0xD5; // x^8 + x^7 + x^6 + x^4 + x^2 + 1
    let mut crc = 0u8;

    for &byte in data {
        crc ^= byte;
        for _ in 0..8 {
            if (crc & 0x80) != 0 {
                crc = (crc << 1) ^ POLYNOMIAL;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

pub struct Frame {
    pub payload_len: u16,
    pub frame_num: u16,
    pub fec_mode: u8, // FEC mode indicator (8, 16, or 32 parity bytes)
    pub payload: Vec<u8>,
    pub payload_crc: u16, // CRC-16 of payload for end-to-end integrity check
}

pub struct FrameEncoder;
pub struct FrameDecoder;

impl FrameEncoder {
    /// Encode frame with header CRC and payload CRC-16 for end-to-end integrity
    pub fn encode(frame: &Frame) -> Result<Vec<u8>> {
        if frame.payload.len() > MAX_PAYLOAD_SIZE {
            return Err(AudioModemError::InvalidFrameSize);
        }

        let mut header = vec![0u8; FRAME_HEADER_SIZE];

        // Write payload length (2 bytes, big-endian)
        header[0] = (frame.payload_len >> 8) as u8;
        header[1] = frame.payload_len as u8;

        // Write frame number (2 bytes, big-endian)
        header[2] = (frame.frame_num >> 8) as u8;
        header[3] = frame.frame_num as u8;

        // Calculate and write CRC-8 of header (excluding CRC field itself)
        let crc_checksum = crc8(&header[..4]);
        header[4] = crc_checksum;

        // FEC mode byte (previously reserved)
        header[5] = frame.fec_mode;

        // Reserved bytes
        header[6] = 0;
        header[7] = 0;

        // Combine header + payload + payload CRC-16
        let mut encoded = header;
        encoded.extend_from_slice(&frame.payload);

        // Calculate and append CRC-16 of payload (2 bytes, big-endian)
        let payload_crc = crc16(&frame.payload);
        encoded.push((payload_crc >> 8) as u8);
        encoded.push(payload_crc as u8);

        Ok(encoded)
    }
}

impl FrameDecoder {
    /// Decode frame header and verify CRC
    pub fn decode_header(data: &[u8]) -> Result<(u16, u16, u8)> {
        if data.len() < FRAME_HEADER_SIZE {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Read payload length
        let payload_len = ((data[0] as u16) << 8) | (data[1] as u16);

        // Read frame number
        let frame_num = ((data[2] as u16) << 8) | (data[3] as u16);

        // Verify CRC
        let expected_crc = data[4];
        let computed_crc = crc8(&data[..4]);

        if expected_crc != computed_crc {
            return Err(AudioModemError::HeaderCrcMismatch);
        }

        // Read FEC mode
        let fec_mode = data[5];

        Ok((payload_len, frame_num, fec_mode))
    }

    /// Decode complete frame (header + payload + payload CRC-16)
    pub fn decode(data: &[u8]) -> Result<Frame> {
        let (payload_len, frame_num, fec_mode) = Self::decode_header(data)?;

        // Need at least: header + payload + 2 bytes for CRC-16
        if data.len() < FRAME_HEADER_SIZE + payload_len as usize + 2 {
            return Err(AudioModemError::InvalidFrameSize);
        }

        let payload_start = FRAME_HEADER_SIZE;
        let payload_end = FRAME_HEADER_SIZE + payload_len as usize;
        let payload = data[payload_start..payload_end].to_vec();

        // Extract CRC-16 from last 2 bytes (big-endian)
        let received_crc = ((data[payload_end] as u16) << 8) | (data[payload_end + 1] as u16);

        // Recalculate CRC-16 over the payload
        let computed_crc = crc16(&payload);

        if received_crc != computed_crc {
            return Err(AudioModemError::PayloadCrcMismatch);
        }

        Ok(Frame {
            payload_len,
            frame_num,
            fec_mode,
            payload,
            payload_crc: computed_crc,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_encode_decode() {
        let payload = b"Hello".to_vec();
        let frame = Frame {
            payload_len: 5,
            frame_num: 1,
            fec_mode: 8,
            payload: payload.clone(),
            payload_crc: crc16(&payload),
        };

        let encoded = FrameEncoder::encode(&frame).unwrap();
        assert!(encoded.len() >= FRAME_HEADER_SIZE + 5 + 2); // +2 for CRC-16

        let decoded = FrameDecoder::decode(&encoded).unwrap();
        assert_eq!(decoded.payload_len, 5);
        assert_eq!(decoded.frame_num, 1);
        assert_eq!(decoded.fec_mode, 8);
        assert_eq!(decoded.payload, b"Hello");
        assert_eq!(decoded.payload_crc, crc16(b"Hello"));
    }

    #[test]
    fn test_frame_header_crc_validation() {
        let payload = b"Hello".to_vec();
        let frame = Frame {
            payload_len: 5,
            frame_num: 1,
            fec_mode: 8,
            payload: payload.clone(),
            payload_crc: crc16(&payload),
        };

        let mut encoded = FrameEncoder::encode(&frame).unwrap();
        // Corrupt header CRC-8
        encoded[4] = encoded[4].wrapping_add(1);

        match FrameDecoder::decode(&encoded) {
            Err(AudioModemError::HeaderCrcMismatch) => {} // Expected
            _ => panic!("Expected HeaderCrcMismatch error"),
        }
    }

    #[test]
    fn test_frame_payload_crc_validation() {
        let payload = b"Hello".to_vec();
        let frame = Frame {
            payload_len: 5,
            frame_num: 1,
            fec_mode: 8,
            payload: payload.clone(),
            payload_crc: crc16(&payload),
        };

        let mut encoded = FrameEncoder::encode(&frame).unwrap();
        // Corrupt payload byte (change 'H' to 'G')
        encoded[FRAME_HEADER_SIZE] = b'G';

        match FrameDecoder::decode(&encoded) {
            Err(AudioModemError::PayloadCrcMismatch) => {} // Expected
            _ => panic!("Expected PayloadCrcMismatch error"),
        }
    }

    #[test]
    fn test_corrupted_payload_rejected() {
        // Test case: "Hello World" (11 bytes)
        let original_payload = b"Hello World".to_vec();
        let frame = Frame {
            payload_len: 11,
            frame_num: 0,
            fec_mode: 8,
            payload: original_payload.clone(),
            payload_crc: crc16(&original_payload),
        };

        let mut encoded = FrameEncoder::encode(&frame).unwrap();

        // Try to pass "Hg|lo World" as corrupted data (same length, different chars)
        // This simulates the bug: if we change 'e' to 'g' and 'l' to '|'
        let corrupted_payload = b"Hg|lo World".to_vec();
        assert_eq!(corrupted_payload.len(), 11); // Same length

        // Replace the payload in the original encoded frame with corrupted payload
        // but keep the wrong CRC that doesn't match the corrupted data
        for i in 0..11 {
            encoded[FRAME_HEADER_SIZE + i] = corrupted_payload[i];
        }
        // The last 2 bytes are still the original CRC which won't match "Hg|lo World"

        // This should fail because the payload CRC won't match
        match FrameDecoder::decode(&encoded) {
            Err(AudioModemError::PayloadCrcMismatch) => {} // Expected - payload was corrupted
            _ => panic!("Expected PayloadCrcMismatch error for corrupted payload"),
        }
    }
}
