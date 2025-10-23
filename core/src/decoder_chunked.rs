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

        // Demodulate all symbols and collect all bits
        let mut all_bits = Vec::new();
        let mut pos = data_start;

        while pos + SAMPLES_PER_SYMBOL <= data_end {
            let symbol_bits = self.ofdm.demodulate(&samples[pos..])?;
            all_bits.extend_from_slice(&symbol_bits);
            pos += SAMPLES_PER_SYMBOL;
        }

        // Convert all bits to bytes
        let mut all_bytes = Vec::new();
        for chunk in all_bits.chunks(8) {
            if chunk.len() == 8 {
                let mut byte = 0u8;
                for (i, &bit) in chunk.iter().enumerate() {
                    if bit {
                        byte |= 1 << (7 - i);
                    }
                }
                all_bytes.push(byte);
            }
        }

        // Process FEC chunks to extract encoded data chunks
        let mut collected_chunks: HashMap<u16, Vec<Chunk>> = HashMap::new();
        let mut total_chunks: Option<u16> = None;

        let mut byte_pos = 0;
        while byte_pos + RS_TOTAL_BYTES <= all_bytes.len() {
            // Decode FEC chunk (takes 255 bytes, returns 223 bytes)
            let fec_chunk = &all_bytes[byte_pos..byte_pos + RS_TOTAL_BYTES];

            if let Ok(decoded_bytes) = self.fec.decode(fec_chunk) {
                // Try to extract chunk from decoded data
                // decoded_bytes is 223 bytes with chunk at start + padding
                // Chunk is: 7 bytes header + chunk_bits/8 bytes data
                let chunk_data_bytes = self.chunk_bits / 8;
                let chunk_total_bytes = 7 + chunk_data_bytes;

                if decoded_bytes.len() >= chunk_total_bytes {
                    // Extract only the chunk portion (ignore padding from FEC)
                    let chunk_portion = &decoded_bytes[0..chunk_total_bytes];

                    // Try to parse chunk header
                    if let Ok(chunk) = Chunk::from_bytes(chunk_portion) {
                        // Validate CRC
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
                                    // Early termination: we have all chunks
                                    return self.reassemble_and_return(&collected_chunks);
                                }
                            }
                        }
                    }
                }
            }

            byte_pos += RS_TOTAL_BYTES;
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

        // Calculate original data length from chunks' payload_len field
        // The payload_len in each chunk header tells us the actual data size (without padding)
        let mut original_len = 0usize;

        for (i, chunk) in chunks_vec.iter().enumerate() {
            if i == chunks_vec.len() - 1 {
                // Last chunk: use payload_len from header (includes padding info)
                original_len += chunk.header.payload_len as usize;
            } else {
                // Non-last chunks: all data is real (no padding)
                original_len += chunk.data.len();
            }
        }

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
