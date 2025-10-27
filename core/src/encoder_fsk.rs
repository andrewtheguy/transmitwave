use crate::error::Result;
use crate::fec::{FecEncoder, FecMode};
use crate::framing::{Frame, FrameEncoder, crc16};
use crate::fsk::{FskModulator, FountainConfig};
use crate::sync::{generate_preamble, generate_postamble_signal};
use crate::{MAX_PAYLOAD_SIZE, PREAMBLE_SAMPLES, POSTAMBLE_SAMPLES};
use raptorq::{Encoder, EncodingPacket};
use std::time::{Duration, Instant};

/// Encoder using Multi-tone FSK with Reed-Solomon FEC
///
/// Uses 6 simultaneous audio frequencies to encode 3 bytes (24 bits) per symbol
/// with non-coherent energy detection (Goertzel algorithm) for maximum reliability
/// in over-the-air transmission scenarios.
///
/// Benefits:
/// - Highly robust to noise and distortion
/// - No phase synchronization required (non-coherent detection)
/// - Well-suited for speaker-to-microphone transmission
/// - Sub-bass frequency band (400-2300 Hz) for excellent room acoustics
/// - Simultaneous multi-tone transmission for redundancy
pub struct EncoderFsk {
    fsk: FskModulator,
    fec: FecEncoder,
}

impl EncoderFsk {
    pub fn new() -> Result<Self> {
        Ok(Self {
            fsk: FskModulator::new(),
            fec: FecEncoder::new()?,
        })
    }

    /// Encode binary data into audio samples using multi-tone FSK modulation
    /// Returns: preamble + (FSK symbols) + postamble
    ///
    /// Each symbol encodes 3 bytes (24 bits) using 6 simultaneous frequencies.
    ///
    /// Uses variable Reed-Solomon parity based on payload size:
    /// - Small payloads (< 20 bytes): 8 parity bytes (75% less overhead)
    /// - Medium payloads (20-50 bytes): 16 parity bytes (50% less overhead)
    /// - Large payloads (> 50 bytes): 32 parity bytes (full protection)
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>> {
        if data.len() > MAX_PAYLOAD_SIZE {
            return Err(crate::error::AudioModemError::InvalidInputSize);
        }

        // Create frame with header and CRC (without FEC mode yet)
        let payload = data.to_vec();

        // Determine FEC mode based on frame size (header + payload + CRC)
        let frame_data_size = 8 + data.len() + 2; // header(8) + payload + crc16(2)
        let fec_mode = FecMode::from_data_size(frame_data_size);

        let frame = Frame {
            payload_len: data.len() as u16,
            frame_num: 0,
            fec_mode: fec_mode.to_u8(),
            payload: payload.clone(),
            payload_crc: crc16(&payload),
        };

        let frame_data = FrameEncoder::encode(&frame)?;

        // Apply variable shortened Reed-Solomon FEC encoding
        let mut encoded_data = Vec::new();

        // Add 2-byte length prefix so decoder knows the frame data length
        let frame_len = frame_data.len() as u16;
        encoded_data.push((frame_len >> 8) as u8);
        encoded_data.push(frame_len as u8);

        for chunk in frame_data.chunks(223) {
            let chunk_len = chunk.len();

            // Shortened RS: prepend zeros, encode, remove zeros
            // This avoids transmitting padding bytes for small payloads
            let padding_needed = 223 - chunk_len;

            // Create padded data for RS encoder
            let mut padded = vec![0u8; padding_needed];
            padded.extend_from_slice(chunk);

            // Encode with variable RS parity based on FEC mode
            let fec_chunk = self.fec.encode_with_mode(&padded, fec_mode)?;

            // Only transmit: actual data + parity (skip the prepended zeros)
            // Parity size depends on FEC mode (8, 16, or 32 bytes)
            encoded_data.extend_from_slice(&fec_chunk[padding_needed..]);
        }

        // Pad encoded data to be a multiple of FSK_BYTES_PER_SYMBOL (3 bytes)
        // Multi-tone FSK transmits 3 bytes per symbol
        let remainder = encoded_data.len() % crate::fsk::FSK_BYTES_PER_SYMBOL;
        if remainder != 0 {
            let padding = crate::fsk::FSK_BYTES_PER_SYMBOL - remainder;
            encoded_data.resize(encoded_data.len() + padding, 0u8);
        }
        // Generate preamble signal for synchronization
        let preamble = generate_preamble(PREAMBLE_SAMPLES, 0.5);

        // Modulate data bytes using multi-tone FSK
        let mut samples = preamble;
        let fsk_samples = self.fsk.modulate(&encoded_data)?;
        samples.extend_from_slice(&fsk_samples);

        // Generate postamble signal for frame boundary detection
        let postamble = generate_postamble_signal(POSTAMBLE_SAMPLES, 0.5);
        samples.extend_from_slice(&postamble);

        Ok(samples)
    }

    /// Encode data using fountain mode for continuous streaming transmission
    ///
    /// Returns a FountainStream iterator that generates unique encoded blocks
    /// continuously until the configured timeout is reached.
    ///
    /// Each yielded Vec<f32> is a complete audio chunk with:
    /// preamble + fountain_block + postamble
    pub fn encode_fountain(&mut self, data: &[u8], config: Option<FountainConfig>) -> Result<FountainStream> {
        if data.len() > MAX_PAYLOAD_SIZE {
            return Err(crate::error::AudioModemError::InvalidInputSize);
        }

        let config = config.unwrap_or_default();

        // Create frame with header and CRC
        let payload = data.to_vec();
        let frame = Frame {
            payload_len: data.len() as u16,
            frame_num: 0,
            fec_mode: 0, // Not used in fountain mode
            payload: payload.clone(),
            payload_crc: crc16(&payload),
        };

        let frame_data = FrameEncoder::encode(&frame)?;

        // Create RaptorQ encoder using with_defaults for proper parameter handling
        let symbol_size = config.block_size as u16;
        let oti = raptorq::ObjectTransmissionInformation::with_defaults(
            frame_data.len() as u64,
            symbol_size
        );

        let encoder = Encoder::new(&frame_data, oti);
        let source_packets = encoder.get_encoded_packets(0);
        if source_packets.is_empty() {
            return Err(crate::error::AudioModemError::InvalidConfig(
                "RaptorQ encoder did not produce any source packets".to_string(),
            ));
        }

        let block_count = encoder.get_block_encoders().len();
        if block_count == 0 {
            return Err(crate::error::AudioModemError::InvalidConfig(
                "RaptorQ encoder has no source blocks".to_string(),
            ));
        }

        let repair_counters = vec![0u32; block_count];
        let repairs_per_cycle = if config.repair_blocks_ratio <= 0.0 {
            0
        } else {
            let desired = (source_packets.len() as f32 * config.repair_blocks_ratio).ceil() as usize;
            desired.max(1)
        };

        // Calculate max samples based on timeout_secs as audio duration
        let sample_rate = 16000; // FSK sample rate
        let max_samples = config.timeout_secs as usize * sample_rate;

        Ok(FountainStream {
            encoder,
            frame_length: frame_data.len(),
            symbol_size,
            fsk: FskModulator::new(),
            config,
            block_id: 0,
            source_packets,
            next_source_idx: 0,
            repair_counters,
            repair_block_cursor: 0,
            repairs_per_cycle,
            repairs_sent_this_cycle: 0,
            total_samples_generated: 0,
            max_samples,
        })
    }
}

/// Iterator that generates continuous fountain-encoded audio blocks
pub struct FountainStream {
    encoder: Encoder,
    frame_length: usize,
    symbol_size: u16,
    fsk: FskModulator,
    config: FountainConfig,
    block_id: u32,
    source_packets: Vec<EncodingPacket>,
    next_source_idx: usize,
    repair_counters: Vec<u32>,
    repair_block_cursor: usize,
    repairs_per_cycle: usize,
    repairs_sent_this_cycle: usize,
    total_samples_generated: usize,
    max_samples: usize,
}

impl Iterator for FountainStream {
    type Item = Vec<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        // Check if we've reached the audio duration limit
        if self.total_samples_generated >= self.max_samples {
            return None;
        }

        // Select next fountain packet (cycles through source packets and then repair packets)
        let packet = match self.select_next_packet() {
            Some(packet) => packet,
            None => return None,
        };
        let packet_data = packet.serialize();

        let mut encoded_data = Vec::new();

        // Include frame metadata in every block so the decoder can resynchronize mid-stream
        encoded_data.extend_from_slice(&(self.frame_length as u32).to_be_bytes());
        encoded_data.extend_from_slice(&self.symbol_size.to_be_bytes());

        // Prefix each block with the serialized packet length so padding can be removed
        let packet_len = packet_data.len() as u16;
        encoded_data.extend_from_slice(&packet_len.to_be_bytes());
        encoded_data.extend_from_slice(&packet_data);
        let remainder = encoded_data.len() % crate::fsk::FSK_BYTES_PER_SYMBOL;
        if remainder != 0 {
            let padding = crate::fsk::FSK_BYTES_PER_SYMBOL - remainder;
            encoded_data.resize(encoded_data.len() + padding, 0u8);
        }
        debug_assert_eq!(encoded_data.len() % crate::fsk::FSK_BYTES_PER_SYMBOL, 0);

        // Generate audio: preamble + FSK data only (no postamble for fountain mode)
        let preamble = generate_preamble(PREAMBLE_SAMPLES, 0.5);
        let mut samples = preamble;

        match self.fsk.modulate(&encoded_data) {
            Ok(fsk_samples) => {
                samples.extend_from_slice(&fsk_samples);
                // No postamble - fountain mode is open-ended with only preamble signaling

                self.total_samples_generated += samples.len();
                self.block_id += 1;
                Some(samples)
            }
            Err(_) => None,
        }
    }
}

impl FountainStream {
    fn select_next_packet(&mut self) -> Option<EncodingPacket> {
        loop {
            if self.next_source_idx < self.source_packets.len() {
                let packet = self.source_packets[self.next_source_idx].clone();
                self.next_source_idx += 1;
                return Some(packet);
            }

            if self.repairs_per_cycle > 0 && self.repairs_sent_this_cycle < self.repairs_per_cycle {
                if let Some(packet) = self.next_repair_packet() {
                    self.repairs_sent_this_cycle += 1;
                    return Some(packet);
                } else {
                    return None;
                }
            }

            if self.source_packets.is_empty() {
                return None;
            }

            // Restart cycle: emit all source packets again, then new repair packets
            self.next_source_idx = 0;
            self.repairs_sent_this_cycle = 0;
        }
    }

    fn next_repair_packet(&mut self) -> Option<EncodingPacket> {
        let block_encoders = self.encoder.get_block_encoders();
        if block_encoders.is_empty() {
            return None;
        }

        if self.repair_counters.len() < block_encoders.len() {
            self.repair_counters.resize(block_encoders.len(), 0);
        }

        if self.repair_block_cursor >= block_encoders.len() {
            self.repair_block_cursor = 0;
        }

        let block_idx = self.repair_block_cursor;
        self.repair_block_cursor = (self.repair_block_cursor + 1) % block_encoders.len();

        if let Some(counter) = self.repair_counters.get_mut(block_idx) {
            let packets = block_encoders[block_idx].repair_packets(*counter, 1);
            if packets.is_empty() {
                return None;
            }
            *counter += 1;
            return packets.into_iter().next();
        }

        None
    }
}

impl Default for EncoderFsk {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_fsk_basic() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Hello";
        let samples = encoder.encode(data).unwrap();

        // Should have: preamble + FSK data + postamble
        assert!(samples.len() > PREAMBLE_SAMPLES + POSTAMBLE_SAMPLES);
    }

    #[test]
    fn test_encoder_fsk_empty_data() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"";
        let result = encoder.encode(data);
        // Empty data should still work (frame header only)
        assert!(result.is_ok());
    }

    #[test]
    fn test_encoder_fsk_max_payload() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = vec![0u8; MAX_PAYLOAD_SIZE];
        let result = encoder.encode(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_encoder_fsk_exceeds_max_payload() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = vec![0u8; MAX_PAYLOAD_SIZE + 1];
        let result = encoder.encode(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_encoder_fsk_structure() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Test";
        let samples = encoder.encode(data).unwrap();

        // Verify preamble is at the start (should be non-zero)
        let preamble_slice = &samples[0..PREAMBLE_SAMPLES];
        let preamble_has_signal = preamble_slice.iter().any(|&s| s.abs() > 0.01);
        assert!(preamble_has_signal, "Preamble should contain signal");

        // Verify postamble is at the end
        let postamble_start = samples.len() - POSTAMBLE_SAMPLES;
        let postamble_slice = &samples[postamble_start..];
        let postamble_has_signal = postamble_slice.iter().any(|&s| s.abs() > 0.01);
        assert!(postamble_has_signal, "Postamble should contain signal");
    }

    #[test]
    fn test_encoder_fsk_deterministic() {
        let mut encoder1 = EncoderFsk::new().unwrap();
        let mut encoder2 = EncoderFsk::new().unwrap();
        let data = b"Deterministic test";

        let samples1 = encoder1.encode(data).unwrap();
        let samples2 = encoder2.encode(data).unwrap();

        // Same input should produce same output
        assert_eq!(samples1.len(), samples2.len());
        for (i, (&s1, &s2)) in samples1.iter().zip(samples2.iter()).enumerate() {
            assert!(
                (s1 - s2).abs() < 1e-6,
                "Mismatch at sample {}: {} vs {}",
                i,
                s1,
                s2
            );
        }
    }

    #[test]
    fn test_fountain_stream_basic() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Fountain test data";

        let config = FountainConfig {
            timeout_secs: 1, // Short timeout for test
            block_size: 32,
            repair_blocks_ratio: 0.5,
        };

        let stream = encoder.encode_fountain(data, Some(config)).unwrap();

        // Generate some blocks
        let blocks: Vec<_> = stream.take(5).collect();

        // Should generate at least some blocks
        assert!(!blocks.is_empty());

        // Each block should contain preamble + data + postamble
        for block in &blocks {
            assert!(block.len() > PREAMBLE_SAMPLES + POSTAMBLE_SAMPLES);
        }
    }

    #[test]
    fn test_fountain_stream_timeout() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Timeout test";

        let config = FountainConfig {
            timeout_secs: 1,
            block_size: 32,
            repair_blocks_ratio: 0.5,
        };

        let stream = encoder.encode_fountain(data, Some(config)).unwrap();

        // Count blocks generated within timeout
        let block_count = stream.count();

        // Should generate some blocks but eventually stop
        assert!(block_count > 0);
        println!("Generated {} blocks in 1 second", block_count);
    }

    #[test]
    fn test_fountain_default_config() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Default config test";

        // Should work with default config
        let mut stream = encoder.encode_fountain(data, None).unwrap();

        // Should generate at least one block
        assert!(stream.next().is_some());
    }
}
