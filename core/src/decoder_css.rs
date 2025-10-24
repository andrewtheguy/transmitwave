use crate::error::{AudioModemError, Result};
use crate::css::CssDemodulator;
use crate::fec::FecDecoder;
use crate::framing::FrameDecoder;
use crate::sync::{detect_postamble, detect_preamble};
use crate::{FRAME_HEADER_SIZE, PREAMBLE_SAMPLES, RS_TOTAL_BYTES, CSS_SAMPLES_PER_SYMBOL};

pub struct DecoderCss {
    css: CssDemodulator,
    fec: FecDecoder,
}

impl DecoderCss {
    pub fn new() -> Result<Self> {
        Ok(Self {
            css: CssDemodulator::new()?,
            fec: FecDecoder::new()?,
        })
    }

    /// Decode audio samples back to binary data using CSS demodulation
    /// Expects: preamble + CSS-modulated data + postamble
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        if samples.len() < CSS_SAMPLES_PER_SYMBOL * 2 {
            return Err(AudioModemError::InsufficientData);
        }

        // Detect preamble
        let preamble_pos = detect_preamble(samples, 500.0)
            .ok_or(AudioModemError::PreambleNotFound)?;

        // Start reading data after preamble
        let data_start = preamble_pos + PREAMBLE_SAMPLES;

        if data_start + CSS_SAMPLES_PER_SYMBOL > samples.len() {
            return Err(AudioModemError::InsufficientData);
        }

        // Try to detect postamble in remaining samples
        let remaining = &samples[data_start..];
        let postamble_pos = detect_postamble(remaining, 100.0)
            .ok_or(AudioModemError::PostambleNotFound)?;

        let data_end = data_start + postamble_pos;

        // Demodulate all CSS symbols between data_start and data_end
        let data_samples = &samples[data_start..data_end];
        let bits = self.css.demodulate(data_samples)?;

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

        // Decode FEC
        let mut decoded_data = Vec::new();
        for chunk in bytes.chunks(RS_TOTAL_BYTES) {
            let fec_result = self.fec.decode(chunk)?;
            decoded_data.extend_from_slice(&fec_result);
        }

        // Decode frame
        let payload = FrameDecoder::decode(&decoded_data)?;
        Ok(payload)
    }
}

impl Default for DecoderCss {
    fn default() -> Self {
        Self::new().unwrap()
    }
}
