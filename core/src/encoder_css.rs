use crate::error::Result;
use crate::css::CssModulator;
use crate::fec::FecEncoder;
use crate::framing::{Frame, FrameEncoder};
use crate::sync::{generate_chirp, generate_postamble};
use crate::{MAX_PAYLOAD_SIZE, PREAMBLE_SAMPLES, POSTAMBLE_SAMPLES};

pub struct EncoderCss {
    css: CssModulator,
    fec: FecEncoder,
}

impl EncoderCss {
    pub fn new() -> Result<Self> {
        Ok(Self {
            css: CssModulator::new()?,
            fec: FecEncoder::new()?,
        })
    }

    /// Encode binary data into audio samples using CSS modulation
    /// Returns: preamble + CSS-modulated data + postamble
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>> {
        if data.len() > MAX_PAYLOAD_SIZE {
            return Err(crate::error::AudioModemError::InvalidInputSize);
        }

        // Create frame
        let frame = Frame {
            payload_len: data.len() as u16,
            frame_num: 0,
            payload: data.to_vec(),
        };

        let frame_data = FrameEncoder::encode(&frame)?;

        // Encode each byte with FEC
        let mut encoded_data = Vec::new();
        for chunk in frame_data.chunks(223) {
            let fec_chunk = self.fec.encode(chunk)?;
            encoded_data.extend_from_slice(&fec_chunk);
        }

        // Convert bytes to bits for CSS
        let mut bits = Vec::new();
        for byte in encoded_data {
            for i in (0..8).rev() {
                bits.push((byte >> i) & 1 == 1);
            }
        }

        // Generate preamble (chirp)
        let preamble = generate_chirp(PREAMBLE_SAMPLES, 200.0, 4000.0, 0.5);

        // Modulate data bits using CSS
        let mut samples = preamble;
        let css_samples = self.css.modulate(&bits)?;
        samples.extend_from_slice(&css_samples);

        // Generate postamble
        let postamble = generate_postamble(POSTAMBLE_SAMPLES, 0.5);
        samples.extend_from_slice(&postamble);

        Ok(samples)
    }
}

impl Default for EncoderCss {
    fn default() -> Self {
        Self::new().unwrap()
    }
}
