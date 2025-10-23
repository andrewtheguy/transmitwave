use crate::chunking::{ChunkDecoder, Chunk};
use crate::error::{AudioModemError, Result};
use crate::fec::FecDecoder;
use crate::ofdm::OfdmDemodulator;
use crate::sync::{detect_postamble, detect_preamble};
use crate::{PREAMBLE_SAMPLES, RS_TOTAL_BYTES, SAMPLES_PER_SYMBOL};
use std::collections::HashMap;

pub struct DecoderChunked {
    ofdm: OfdmDemodulator,
    fec: FecDecoder,
    chunk_decoder: ChunkDecoder,
    chunk_bits: usize,
}

impl DecoderChunked {
    /// Create new chunked decoder
    pub fn new(chunk_bits: usize) -> Result<Self> {
        Ok(Self {
            ofdm: OfdmDemodulator::new(),
            fec: FecDecoder::new()?,
            chunk_decoder: ChunkDecoder::new(chunk_bits)?,
            chunk_bits,
        })
    }

    /// Decode audio samples with streaming chunk validation and early termination
    /// Returns payload as soon as all chunks are successfully decoded
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        if samples.len() < SAMPLES_PER_SYMBOL * 2 {
            return Err(AudioModemError::InsufficientData);
        }

        // Detect preamble
        let preamble_pos = detect_preamble(samples, 500.0)
            .ok_or(AudioModemError::PreambleNotFound)?;

        // Start reading data after preamble
        let data_start = preamble_pos + PREAMBLE_SAMPLES;

        if data_start + SAMPLES_PER_SYMBOL > samples.len() {
            return Err(AudioModemError::InsufficientData);
        }

        // Try to detect postamble in remaining samples
        let remaining = &samples[data_start..];
        let postamble_pos = detect_postamble(remaining, 100.0)
            .ok_or(AudioModemError::PostambleNotFound)?;

        let data_end = data_start + postamble_pos;

        // Demodulate and collect chunks
        let mut collected_chunks: HashMap<u16, Vec<Chunk>> = HashMap::new();
        let mut total_chunks: Option<u16> = None;
        let mut pos = data_start;

        // Stream through OFDM symbols
        while pos + SAMPLES_PER_SYMBOL <= data_end {
            let symbol_bits = self.ofdm.demodulate(&samples[pos..])?;
            pos += SAMPLES_PER_SYMBOL;

            // Try to decode chunk from accumulated bits
            if let Ok(chunk) = self.try_decode_chunk(&symbol_bits) {
                // Validate chunk CRC
                if chunk.validate_crc() {
                    // Update total_chunks from first valid chunk
                    if total_chunks.is_none() {
                        total_chunks = Some(chunk.header.total_chunks);
                    }

                    let chunk_id = chunk.header.chunk_id;

                    // Store chunk
                    collected_chunks
                        .entry(chunk_id)
                        .or_insert_with(Vec::new)
                        .push(chunk);

                    // Check if we have all chunks needed for early termination
                    if let Some(total) = total_chunks {
                        if self.have_all_chunks(&collected_chunks, total) {
                            // Early termination: we have all chunks, exit before postamble
                            return self.reassemble_and_return(&collected_chunks);
                        }
                    }
                }
            }
        }

        // If we reach here, try to reassemble from whatever chunks we collected
        if let Some(total) = total_chunks {
            if self.have_all_chunks(&collected_chunks, total) {
                return self.reassemble_and_return(&collected_chunks);
            }
        }

        // Not enough chunks collected
        Err(AudioModemError::InvalidFrameSize)
    }

    /// Try to decode a chunk from bits (this needs to be built from bit stream)
    fn try_decode_chunk(&self, symbol_bits: &[bool]) -> Result<Chunk> {
        // Convert bits to bytes
        let mut bytes = Vec::new();
        for chunk in symbol_bits.chunks(8) {
            if chunk.len() == 8 {
                let mut byte = 0u8;
                for (i, &bit) in chunk.iter().enumerate() {
                    if bit {
                        byte |= 1 << (7 - i);
                    }
                }
                bytes.push(byte);
            }
        }

        // Pad to multiple of RS_TOTAL_BYTES for FEC decoding
        while bytes.len() % RS_TOTAL_BYTES != 0 {
            bytes.push(0);
        }

        if bytes.len() < RS_TOTAL_BYTES {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Decode first FEC chunk (one chunk = 6 bytes header + 4/6/8 bytes data)
        let fec_chunk = &bytes[0..RS_TOTAL_BYTES];
        let decoded_chunk = self.fec.decode(fec_chunk)?;

        // Decode chunk from bytes
        Chunk::from_bytes(&decoded_chunk)
    }

    /// Check if we have all chunks needed (0..total_chunks-1)
    fn have_all_chunks(&self, collected: &HashMap<u16, Vec<Chunk>>, total: u16) -> bool {
        for i in 0..total {
            if !collected.contains_key(&i) {
                return false;
            }
        }
        true
    }

    /// Reassemble chunks and return the original data
    fn reassemble_and_return(&self, collected: &HashMap<u16, Vec<Chunk>>) -> Result<Vec<u8>> {
        if collected.is_empty() {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Get total chunks from any chunk
        let total_chunks = collected
            .values()
            .next()
            .ok_or(AudioModemError::InvalidFrameSize)?[0]
            .header
            .total_chunks;

        // Collect one valid copy of each chunk
        let mut chunks_vec = Vec::new();
        for i in 0..total_chunks {
            let chunk_copies = &collected[&i];
            // Use first valid copy (they should all be identical)
            chunks_vec.push(chunk_copies[0].clone());
        }

        // Sort by chunk_id to reassemble in order
        chunks_vec.sort_by_key(|c| c.header.chunk_id);

        // Calculate original data length from last chunk
        // This is a simplified approach - in a real system, you'd store the length
        let original_len = chunks_vec
            .iter()
            .map(|c| c.data.len())
            .sum::<usize>();

        self.chunk_decoder.reassemble_chunks(&chunks_vec, original_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_chunked_new() {
        let decoder = DecoderChunked::new(48);
        assert!(decoder.is_ok());
    }

    #[test]
    fn test_decoder_chunked_invalid_chunk_bits() {
        let decoder = DecoderChunked::new(40);
        assert!(decoder.is_err());
    }

    #[test]
    fn test_decoder_have_all_chunks_empty() {
        let decoder = DecoderChunked::new(48).unwrap();
        let chunks: HashMap<u16, Vec<Chunk>> = HashMap::new();
        assert!(!decoder.have_all_chunks(&chunks, 1));
    }

    #[test]
    fn test_decoder_have_all_chunks_complete() {
        let decoder = DecoderChunked::new(48).unwrap();
        let mut chunks: HashMap<u16, Vec<Chunk>> = HashMap::new();

        let chunk0 = Chunk::new(0, 2, vec![1, 2, 3, 4, 5, 6]);
        let chunk1 = Chunk::new(1, 2, vec![7, 8, 9, 10, 11, 12]);

        chunks.insert(0, vec![chunk0]);
        chunks.insert(1, vec![chunk1]);

        assert!(decoder.have_all_chunks(&chunks, 2));
    }

    #[test]
    fn test_decoder_have_all_chunks_incomplete() {
        let decoder = DecoderChunked::new(48).unwrap();
        let mut chunks: HashMap<u16, Vec<Chunk>> = HashMap::new();

        let chunk0 = Chunk::new(0, 3, vec![1, 2, 3, 4, 5, 6]);
        chunks.insert(0, vec![chunk0]);

        assert!(!decoder.have_all_chunks(&chunks, 3));
    }
}
