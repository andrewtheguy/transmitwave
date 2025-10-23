use crate::error::{AudioModemError, Result};
use crate::fec::FecDecoder;
use crate::framing::FrameDecoder;
use crate::ofdm::OfdmDemodulator;
use crate::sync::{detect_postamble, detect_preamble};
use crate::{FRAME_HEADER_SIZE, PREAMBLE_SAMPLES, RS_TOTAL_BYTES, SAMPLES_PER_SYMBOL};

pub struct Decoder {
    ofdm: OfdmDemodulator,
    fec: FecDecoder,
}

impl Decoder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            ofdm: OfdmDemodulator::new(),
            fec: FecDecoder::new()?,
        })
    }

    /// Decode audio samples back to binary data
    /// Expects: preamble + frame data + postamble
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

        // Demodulate all symbols between data_start and data_end
        let mut bits = Vec::new();
        let mut pos = data_start;

        while pos + SAMPLES_PER_SYMBOL <= data_end {
            let symbol_bits = self.ofdm.demodulate(&samples[pos..])?;
            bits.extend_from_slice(&symbol_bits);
            pos += SAMPLES_PER_SYMBOL;
        }

        if bits.is_empty() {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Convert bits back to bytes
        let mut bytes = Vec::new();
        for chunk in bits.chunks(8) {
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

        // Pad bytes to multiple of RS_TOTAL_BYTES for FEC decoding
        while bytes.len() % RS_TOTAL_BYTES != 0 && bytes.len() < FRAME_HEADER_SIZE + 256 {
            bytes.push(0);
        }

        if bytes.len() < RS_TOTAL_BYTES {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Decode FEC chunks
        let mut decoded_data = Vec::new();
        for chunk in bytes.chunks(RS_TOTAL_BYTES) {
            if chunk.len() == RS_TOTAL_BYTES {
                let decoded_chunk = self.fec.decode(chunk)?;
                decoded_data.extend_from_slice(&decoded_chunk);
            }
        }

        if decoded_data.len() < FRAME_HEADER_SIZE {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Decode frame header and payload
        let frame = FrameDecoder::decode(&decoded_data)?;

        Ok(frame.payload)
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new().unwrap()
    }
}
