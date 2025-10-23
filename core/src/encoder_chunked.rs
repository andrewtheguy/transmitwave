use crate::chunking::{ChunkEncoder, interleave_chunks};
use crate::error::Result;
use crate::fec::FecEncoder;
use crate::ofdm::OfdmModulator;
use crate::sync::{generate_chirp, generate_postamble};
use crate::{PREAMBLE_SAMPLES, POSTAMBLE_SAMPLES};

pub struct EncoderChunked {
    ofdm: OfdmModulator,
    fec: FecEncoder,
    chunk_encoder: ChunkEncoder,
    chunk_bits: usize,
    interleave_factor: usize,
}

impl EncoderChunked {
    /// Create new chunked encoder
    /// chunk_bits: 32, 48, or 64 bits per chunk
    /// interleave_factor: how many times to repeat each chunk (2-5 recommended)
    pub fn new(chunk_bits: usize, interleave_factor: usize) -> Result<Self> {
        Ok(Self {
            ofdm: OfdmModulator::new(),
            fec: FecEncoder::new()?,
            chunk_encoder: ChunkEncoder::new(chunk_bits)?,
            chunk_bits,
            interleave_factor,
        })
    }

    /// Encode binary data into audio samples with chunking and interleaving
    /// Returns: preamble + (interleaved chunk data) + postamble
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>> {
        // Split data into chunks
        let chunks = self.chunk_encoder.split_into_chunks(data);

        // Interleave chunks for redundancy
        let interleaved_chunks = interleave_chunks(chunks, self.interleave_factor);

        // Generate preamble
        let preamble = generate_chirp(PREAMBLE_SAMPLES, 200.0, 4000.0, 0.5);

        let mut samples = preamble;

        // Encode each chunk
        for chunk in interleaved_chunks {
            let chunk_bytes = chunk.to_bytes();

            // Encode chunk with FEC
            let mut encoded_data = Vec::new();
            for chunk_chunk in chunk_bytes.chunks(223) {
                let fec_chunk = self.fec.encode(chunk_chunk)?;
                encoded_data.extend_from_slice(&fec_chunk);
            }

            // Convert bytes to bits
            let mut bits = Vec::new();
            for byte in encoded_data {
                for i in (0..8).rev() {
                    bits.push((byte >> i) & 1 == 1);
                }
            }

            // Modulate data bits to OFDM symbols
            for symbol_bits in bits.chunks(48) {
                let symbol_samples = self.ofdm.modulate(symbol_bits)?;
                samples.extend_from_slice(&symbol_samples);
            }
        }

        // Generate postamble
        let postamble = generate_postamble(POSTAMBLE_SAMPLES, 0.5);
        samples.extend_from_slice(&postamble);

        Ok(samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_chunked_new() {
        let encoder = EncoderChunked::new(48, 3);
        assert!(encoder.is_ok());
    }

    #[test]
    fn test_encoder_chunked_invalid_chunk_bits() {
        let encoder = EncoderChunked::new(40, 3); // Invalid: must be 32, 48, or 64
        assert!(encoder.is_err());
    }

    #[test]
    fn test_encoder_chunked_small_data_32bits() {
        let mut encoder = EncoderChunked::new(32, 2).unwrap();
        let data = b"Hi";
        let samples = encoder.encode(data);
        assert!(samples.is_ok());
        let audio = samples.unwrap();
        assert!(audio.len() > 0);
    }

    #[test]
    fn test_encoder_chunked_small_data_48bits() {
        let mut encoder = EncoderChunked::new(48, 2).unwrap();
        let data = b"Hello";
        let samples = encoder.encode(data);
        assert!(samples.is_ok());
        let audio = samples.unwrap();
        assert!(audio.len() > 0);
    }

    #[test]
    fn test_encoder_chunked_small_data_64bits() {
        let mut encoder = EncoderChunked::new(64, 2).unwrap();
        let data = b"Testing!";
        let samples = encoder.encode(data);
        assert!(samples.is_ok());
        let audio = samples.unwrap();
        assert!(audio.len() > 0);
    }

    #[test]
    fn test_encoder_chunked_with_interleaving() {
        let mut encoder = EncoderChunked::new(48, 3).unwrap(); // 3x redundancy
        let data = b"Hello";
        let samples = encoder.encode(data);
        assert!(samples.is_ok());
        let audio = samples.unwrap();
        assert!(audio.len() > 0);
    }

    #[test]
    fn test_encoder_chunked_larger_data() {
        let mut encoder = EncoderChunked::new(32, 2).unwrap();
        let data = b"This is a longer test message with multiple chunks";
        let samples = encoder.encode(data);
        assert!(samples.is_ok());
        let audio = samples.unwrap();
        assert!(audio.len() > 0);
    }
}
