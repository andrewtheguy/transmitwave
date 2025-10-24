use crate::error::Result;
use crate::fec::FecEncoder;
use crate::framing::{Frame, FrameEncoder};
use crate::ofdm::OfdmModulator;
use crate::sync::{generate_chirp, generate_postamble};
use crate::{MAX_PAYLOAD_SIZE, PREAMBLE_SAMPLES, POSTAMBLE_SAMPLES, NUM_SUBCARRIERS};

pub struct Encoder {
    ofdm: OfdmModulator,
    fec: FecEncoder,
}

impl Encoder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            ofdm: OfdmModulator::new(),
            fec: FecEncoder::new()?,
        })
    }

    /// Encode binary data into audio samples
    /// Returns: preamble + (frame data with sync) + postamble
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

        // Convert bytes to bits for OFDM
        let mut bits = Vec::new();
        for byte in encoded_data {
            for i in (0..8).rev() {
                bits.push((byte >> i) & 1 == 1);
            }
        }

        // Generate preamble (chirp)
        let preamble = generate_chirp(PREAMBLE_SAMPLES, 200.0, 4000.0, 0.5);

        // Modulate data bits to OFDM symbols
        let mut samples = preamble;

        // Process bits in OFDM symbol chunks (NUM_SUBCARRIERS bits per symbol)
        for symbol_bits in bits.chunks(NUM_SUBCARRIERS) {
            let symbol_samples = self.ofdm.modulate(symbol_bits)?;
            samples.extend_from_slice(&symbol_samples);
        }

        // Generate postamble
        let postamble = generate_postamble(POSTAMBLE_SAMPLES, 0.5);
        samples.extend_from_slice(&postamble);

        Ok(samples)
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new().unwrap()
    }
}
