use crate::error::{AudioModemError, Result};
use crate::{FRAME_HEADER_SIZE, MAX_PAYLOAD_SIZE};

fn simple_crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &byte in data {
        crc = crc.wrapping_add(byte);
        crc = crc.wrapping_mul(17);
    }
    crc
}

pub struct Frame {
    pub payload_len: u16,
    pub frame_num: u16,
    pub payload: Vec<u8>,
}

pub struct FrameEncoder;
pub struct FrameDecoder;

impl FrameEncoder {
    /// Encode frame with header and CRC
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
        let crc_checksum = simple_crc8(&header[..4]);
        header[4] = crc_checksum;

        // Reserved bytes
        header[5] = 0;
        header[6] = 0;
        header[7] = 0;

        // Combine header + payload
        let mut encoded = header;
        encoded.extend_from_slice(&frame.payload);

        Ok(encoded)
    }
}

impl FrameDecoder {
    /// Decode frame header and verify CRC
    pub fn decode_header(data: &[u8]) -> Result<(u16, u16)> {
        if data.len() < FRAME_HEADER_SIZE {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Read payload length
        let payload_len = ((data[0] as u16) << 8) | (data[1] as u16);

        // Read frame number
        let frame_num = ((data[2] as u16) << 8) | (data[3] as u16);

        // Verify CRC
        let expected_crc = data[4];
        let computed_crc = simple_crc8(&data[..4]);

        if expected_crc != computed_crc {
            return Err(AudioModemError::CrcMismatch);
        }

        Ok((payload_len, frame_num))
    }

    /// Decode complete frame (header + payload)
    pub fn decode(data: &[u8]) -> Result<Frame> {
        let (payload_len, frame_num) = Self::decode_header(data)?;

        if data.len() < FRAME_HEADER_SIZE + payload_len as usize {
            return Err(AudioModemError::InvalidFrameSize);
        }

        let payload = data[FRAME_HEADER_SIZE..FRAME_HEADER_SIZE + payload_len as usize].to_vec();

        Ok(Frame {
            payload_len,
            frame_num,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_encode_decode() {
        let frame = Frame {
            payload_len: 5,
            frame_num: 1,
            payload: b"Hello".to_vec(),
        };

        let encoded = FrameEncoder::encode(&frame).unwrap();
        assert!(encoded.len() >= FRAME_HEADER_SIZE + 5);

        let decoded = FrameDecoder::decode(&encoded).unwrap();
        assert_eq!(decoded.payload_len, 5);
        assert_eq!(decoded.frame_num, 1);
        assert_eq!(decoded.payload, b"Hello");
    }

    #[test]
    fn test_frame_crc_validation() {
        let frame = Frame {
            payload_len: 5,
            frame_num: 1,
            payload: b"Hello".to_vec(),
        };

        let mut encoded = FrameEncoder::encode(&frame).unwrap();
        // Corrupt CRC
        encoded[4] = encoded[4].wrapping_add(1);

        assert!(FrameDecoder::decode(&encoded).is_err());
    }
}
