use crate::error::{AudioModemError, Result};

/// CRC-16-CCITT calculation for chunk validation
fn calculate_crc16(data: &[u8]) -> u16 {
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

/// Chunk header: chunk_id (16 bits) + total_chunks (16 bits) + payload_len (8 bits) + crc16 (16 bits)
#[derive(Clone, Debug)]
pub struct ChunkHeader {
    pub chunk_id: u16,
    pub total_chunks: u16,
    pub payload_len: u8, // Original payload length (for last chunk to know padding)
    pub crc16: u16,
}

impl ChunkHeader {
    /// Create new chunk header
    pub fn new(chunk_id: u16, total_chunks: u16, payload_len: u8, crc16: u16) -> Self {
        Self {
            chunk_id,
            total_chunks,
            payload_len,
            crc16,
        }
    }

    /// Encode header to bytes (7 bytes total)
    pub fn to_bytes(&self) -> Vec<u8> {
        vec![
            (self.chunk_id >> 8) as u8,
            self.chunk_id as u8,
            (self.total_chunks >> 8) as u8,
            self.total_chunks as u8,
            self.payload_len,
            (self.crc16 >> 8) as u8,
            self.crc16 as u8,
        ]
    }

    /// Decode header from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 7 {
            return Err(AudioModemError::InvalidFrameSize);
        }
        let chunk_id = ((data[0] as u16) << 8) | (data[1] as u16);
        let total_chunks = ((data[2] as u16) << 8) | (data[3] as u16);
        let payload_len = data[4];
        let crc16 = ((data[5] as u16) << 8) | (data[6] as u16);
        Ok(Self {
            chunk_id,
            total_chunks,
            payload_len,
            crc16,
        })
    }
}

/// A chunk with header and data
#[derive(Clone, Debug)]
pub struct Chunk {
    pub header: ChunkHeader,
    pub data: Vec<u8>,
}

impl Chunk {
    /// Create new chunk with header and data
    pub fn new(chunk_id: u16, total_chunks: u16, data: Vec<u8>) -> Self {
        let crc16 = calculate_crc16(&data);
        let payload_len = data.len() as u8;
        let header = ChunkHeader::new(chunk_id, total_chunks, payload_len, crc16);
        Self { header, data }
    }

    /// Create a chunk with explicit payload length (for tracking unpadded size)
    pub fn with_payload_len(chunk_id: u16, total_chunks: u16, data: Vec<u8>, actual_len: usize) -> Self {
        let crc16 = calculate_crc16(&data);
        let payload_len = (actual_len.min(255)) as u8;
        let header = ChunkHeader::new(chunk_id, total_chunks, payload_len, crc16);
        Self { header, data }
    }

    /// Validate chunk CRC
    pub fn validate_crc(&self) -> bool {
        let calculated_crc = calculate_crc16(&self.data);
        calculated_crc == self.header.crc16
    }

    /// Encode chunk to bytes (header + data)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = self.header.to_bytes();
        result.extend_from_slice(&self.data);
        result
    }

    /// Decode chunk from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 7 {
            return Err(AudioModemError::InvalidFrameSize);
        }
        let header = ChunkHeader::from_bytes(&data[0..7])?;
        let chunk_data = data[7..].to_vec();
        Ok(Self {
            header,
            data: chunk_data,
        })
    }
}

/// Chunk encoder: splits payload into chunks with headers
pub struct ChunkEncoder {
    chunk_bits: usize,
}

impl ChunkEncoder {
    /// Create new chunk encoder
    /// chunk_bits: 32, 48, or 64 bits per chunk
    pub fn new(chunk_bits: usize) -> Result<Self> {
        if ![32, 48, 64].contains(&chunk_bits) {
            return Err(AudioModemError::InvalidConfig(
                "chunk_bits must be 32, 48, or 64".to_string(),
            ));
        }
        Ok(Self { chunk_bits })
    }

    /// Split payload into chunks
    pub fn split_into_chunks(&self, data: &[u8]) -> Vec<Chunk> {
        let chunk_bytes = self.chunk_bits / 8;
        let mut chunks = Vec::new();
        let total_chunks = (data.len() + chunk_bytes - 1) / chunk_bytes;

        for i in 0..total_chunks {
            let start = i * chunk_bytes;
            let end = std::cmp::min(start + chunk_bytes, data.len());
            let actual_len = end - start; // Original length before padding
            let mut chunk_data = data[start..end].to_vec();

            // Pad with zeros to chunk_bytes
            while chunk_data.len() < chunk_bytes {
                chunk_data.push(0);
            }

            // Use with_payload_len to store the actual unpadded length
            let chunk = Chunk::with_payload_len(i as u16, total_chunks as u16, chunk_data, actual_len);
            chunks.push(chunk);
        }

        chunks
    }
}

/// Chunk decoder: validates and reassembles chunks
pub struct ChunkDecoder {
    chunk_bits: usize,
}

impl ChunkDecoder {
    /// Create new chunk decoder
    pub fn new(chunk_bits: usize) -> Result<Self> {
        if ![32, 48, 64].contains(&chunk_bits) {
            return Err(AudioModemError::InvalidConfig(
                "chunk_bits must be 32, 48, or 64".to_string(),
            ));
        }
        Ok(Self { chunk_bits })
    }

    /// Reassemble chunks into original data
    pub fn reassemble_chunks(&self, chunks: &[Chunk], total_len: usize) -> Result<Vec<u8>> {
        if chunks.is_empty() {
            return Err(AudioModemError::InvalidFrameSize);
        }

        let mut result = Vec::new();

        for chunk in chunks {
            if !chunk.validate_crc() {
                return Err(AudioModemError::HeaderCrcMismatch);
            }
            result.extend_from_slice(&chunk.data);
        }

        // Truncate to original length (remove padding)
        result.truncate(total_len);
        Ok(result)
    }
}

/// Interleave chunks for redundancy
pub fn interleave_chunks(chunks: Vec<Chunk>, factor: usize) -> Vec<Chunk> {
    let mut interleaved = Vec::new();
    for _ in 0..factor {
        interleaved.extend_from_slice(&chunks);
    }
    interleaved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc16_calculation() {
        let data = b"Hello";
        let crc1 = calculate_crc16(data);
        let crc2 = calculate_crc16(data);
        assert_eq!(crc1, crc2);
    }

    #[test]
    fn test_crc16_different_data() {
        let data1 = b"Hello";
        let data2 = b"World";
        let crc1 = calculate_crc16(data1);
        let crc2 = calculate_crc16(data2);
        assert_ne!(crc1, crc2);
    }

    #[test]
    fn test_chunk_header_encode_decode() {
        let header = ChunkHeader::new(5, 10, 6, 0xABCD);
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), 7);

        let decoded = ChunkHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.chunk_id, 5);
        assert_eq!(decoded.total_chunks, 10);
        assert_eq!(decoded.payload_len, 6);
        assert_eq!(decoded.crc16, 0xABCD);
    }

    #[test]
    fn test_chunk_encode_decode() {
        let chunk = Chunk::new(2, 5, b"Test".to_vec());
        let bytes = chunk.to_bytes();

        let decoded = Chunk::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.header.chunk_id, 2);
        assert_eq!(decoded.header.total_chunks, 5);
        assert_eq!(decoded.data, b"Test");
        assert!(decoded.validate_crc());
    }

    #[test]
    fn test_chunk_crc_validation() {
        let chunk = Chunk::new(1, 3, b"Hello".to_vec());
        assert!(chunk.validate_crc());
    }

    #[test]
    fn test_chunk_split_32bits() {
        let encoder = ChunkEncoder::new(32).unwrap();
        let data = b"Hi";
        let chunks = encoder.split_into_chunks(data);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].header.chunk_id, 0);
        assert_eq!(chunks[0].header.total_chunks, 1);
        assert!(chunks[0].validate_crc());
    }

    #[test]
    fn test_chunk_split_48bits() {
        let encoder = ChunkEncoder::new(48).unwrap();
        let data = b"Hello";
        let chunks = encoder.split_into_chunks(data);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].data.len(), 6);
        assert!(chunks[0].validate_crc());
    }

    #[test]
    fn test_chunk_split_64bits() {
        let encoder = ChunkEncoder::new(64).unwrap();
        let data = b"Testing!";
        let chunks = encoder.split_into_chunks(data);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].data.len(), 8);
        assert!(chunks[0].validate_crc());
    }

    #[test]
    fn test_chunk_split_multiple() {
        let encoder = ChunkEncoder::new(32).unwrap();
        let data = b"Hello, World!";
        let chunks = encoder.split_into_chunks(data);

        assert_eq!(chunks.len(), 4); // 13 bytes / 4 = 4 chunks
        for chunk in &chunks {
            assert!(chunk.validate_crc());
        }
    }

    #[test]
    fn test_interleave_chunks() {
        let encoder = ChunkEncoder::new(32).unwrap();
        let data = b"ABC";
        let chunks = encoder.split_into_chunks(data);
        let original_count = chunks.len();

        let interleaved = interleave_chunks(chunks, 3);
        assert_eq!(interleaved.len(), original_count * 3);
    }

    #[test]
    fn test_chunk_decode_invalid_size() {
        let decoder = ChunkDecoder::new(32).unwrap();
        let result = decoder.reassemble_chunks(&[], 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_chunk_roundtrip() {
        let encoder = ChunkEncoder::new(48).unwrap();
        let original_data = b"Hello";

        let chunks = encoder.split_into_chunks(original_data);
        assert_eq!(chunks.len(), 1);

        let decoder = ChunkDecoder::new(48).unwrap();
        let reassembled =
            decoder.reassemble_chunks(&chunks, original_data.len()).unwrap();
        assert_eq!(reassembled, original_data);
    }
}
