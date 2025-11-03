use crate::error::Result;
use crate::fec::{FecEncoder, FecMode};
use crate::framing::{Frame, FrameEncoder, crc16};
use crate::fsk::{FskModulator, FountainConfig};
use crate::sync::{generate_preamble, generate_postamble_signal};
use crate::{MAX_PAYLOAD_SIZE, PREAMBLE_SAMPLES, POSTAMBLE_SAMPLES, SYNC_SILENCE_SAMPLES};
use raptorq::{Encoder, EncodingPacket};

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
    /// Returns: silence + preamble + silence + FSK data + silence + postamble + silence
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

        // Build frame: silence → preamble → silence → FSK payload → silence → postamble → silence
        let mut samples = Vec::new();

        // Add silence before preamble for clean frame start
        samples.extend_from_slice(&vec![0.0f32; SYNC_SILENCE_SAMPLES]);

        // Add preamble for synchronization
        samples.extend_from_slice(&preamble);

        // Add silence after preamble for symmetry and clear frame boundaries
        samples.extend_from_slice(&vec![0.0f32; SYNC_SILENCE_SAMPLES]);

        // Modulate data bytes using multi-tone FSK
        let fsk_samples = self.fsk.modulate(&encoded_data)?;
        samples.extend_from_slice(&fsk_samples);

        // Add silence before postamble to separate payload from end marker
        samples.extend_from_slice(&vec![0.0f32; SYNC_SILENCE_SAMPLES]);

        // Generate postamble signal for frame boundary detection
        let postamble = generate_postamble_signal(POSTAMBLE_SAMPLES, 0.5);
        samples.extend_from_slice(&postamble);

        // Add silence after postamble for clean frame end
        samples.extend_from_slice(&vec![0.0f32; SYNC_SILENCE_SAMPLES]);

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

        // Validate block_size before casting to u16
        let symbol_size = u16::try_from(config.block_size)
            .map_err(|_| crate::error::AudioModemError::InvalidConfig(
                format!("block_size {} exceeds maximum u16 value ({})", config.block_size, u16::MAX)
            ))?;

        // Create RaptorQ encoder using with_defaults for proper parameter handling
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
        // Use the single source of truth: crate::SAMPLE_RATE
        let max_samples = if config.timeout_secs == 0 {
            usize::MAX
        } else {
            config.timeout_secs as usize * crate::SAMPLE_RATE
        };

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
        // Check if we've already reached the audio duration limit
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

        // Add CRC-16 checksum of the RaptorQ packet for early corruption detection
        let packet_crc = crc16(&packet_data);
        encoded_data.extend_from_slice(&packet_crc.to_be_bytes());

        let remainder = encoded_data.len() % crate::fsk::FSK_BYTES_PER_SYMBOL;
        if remainder != 0 {
            let padding = crate::fsk::FSK_BYTES_PER_SYMBOL - remainder;
            encoded_data.resize(encoded_data.len() + padding, 0u8);
        }
        assert_eq!(
            encoded_data.len() % crate::fsk::FSK_BYTES_PER_SYMBOL,
            0,
            "FSK symbol alignment invariant violated: encoded_data length ({}) is not a multiple of FSK_BYTES_PER_SYMBOL ({})",
            encoded_data.len(),
            crate::fsk::FSK_BYTES_PER_SYMBOL
        );

        // Generate audio: preamble + FSK data only (no postamble for fountain mode)
        let preamble = generate_preamble(PREAMBLE_SAMPLES, 0.5);
        let mut samples = preamble;

        match self.fsk.modulate(&encoded_data) {
            Ok(fsk_samples) => {
                samples.extend_from_slice(&fsk_samples);
                // No postamble - fountain mode is open-ended with only preamble signaling

                // Always emit complete blocks without truncation, as truncating mid-block creates
                // malformed audio that cannot be deserialized. The max_samples limit is
                // approximate and may be exceeded by one block, which is acceptable.
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
    use crate::SAMPLE_RATE;
    use log::info;

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

        // Each block should contain preamble + data (no postamble in fountain mode)
        for block in &blocks {
            assert!(block.len() > PREAMBLE_SAMPLES);
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
        info!("Generated {} blocks in 1 second", block_count);
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

    #[test]
    fn test_fountain_respects_max_samples() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Max samples test";

        let config = FountainConfig {
            timeout_secs: 10, // Long timeout
            block_size: 32,
            repair_blocks_ratio: 0.5,
        };

        let stream = encoder.encode_fountain(data, Some(config)).unwrap();
        let max_samples = stream.max_samples;

        // Generate all blocks and verify total doesn't greatly exceed max_samples
        // Note: May exceed by one block since we emit complete blocks without truncation
        let total: usize = stream
            .map(|block| block.len())
            .sum();

        // Allow some overshoot - one additional block beyond max_samples is acceptable
        // since we emit complete blocks and never truncate mid-block
        let max_allowed = max_samples + (50 * crate::fsk::FSK_SYMBOL_SAMPLES);

        assert!(
            total <= max_allowed,
            "Total samples {} far exceeds max_samples {} with allowance ({} max)",
            total,
            max_samples,
            max_allowed
        );
    }

    #[test]
    fn test_fountain_block_size_exceeds_u16_max() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Block size test";

        // Create a config with block_size that exceeds u16::MAX
        let config = FountainConfig {
            timeout_secs: 1,
            block_size: u16::MAX as usize + 1, // 65536
            repair_blocks_ratio: 0.5,
        };

        let result = encoder.encode_fountain(data, Some(config));

        // Should return an error due to invalid block_size
        assert!(result.is_err());

        // Verify the error message mentions the issue
        if let Err(err) = result {
            let err_msg = format!("{:?}", err);
            assert!(
                err_msg.contains("block_size") || err_msg.contains("exceeds"),
                "Error should mention block_size or exceeds: {}",
                err_msg
            );
        }
    }

    #[test]
    fn test_fountain_block_size_at_u16_max() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Block size at max test";

        // Create a config with block_size exactly at u16::MAX
        let config = FountainConfig {
            timeout_secs: 1,
            block_size: u16::MAX as usize,
            repair_blocks_ratio: 0.5,
        };

        // Should succeed with u16::MAX
        let result = encoder.encode_fountain(data, Some(config));
        assert!(result.is_ok(), "Should accept block_size at u16::MAX");
    }

    #[test]
    fn test_fountain_uses_sample_rate_constant() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Sample rate test";

        let timeout_secs = 5;
        let config = FountainConfig {
            timeout_secs,
            block_size: 32,
            repair_blocks_ratio: 0.5,
        };

        let stream = encoder.encode_fountain(data, Some(config)).unwrap();

        // Verify that max_samples is calculated using crate::SAMPLE_RATE
        let expected_max_samples = timeout_secs as usize * SAMPLE_RATE;
        assert_eq!(
            stream.max_samples, expected_max_samples,
            "max_samples should be calculated as timeout_secs * SAMPLE_RATE ({} * {} = {})",
            timeout_secs, SAMPLE_RATE, expected_max_samples
        );
    }

    #[test]
    fn test_fountain_repair_packets_have_unique_data() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Unique repair packet test - verify different packets have different encoded data";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 64,
            repair_blocks_ratio: 1.0, // 100% repair overhead for more repair packets
        };

        // Test by checking the underlying RaptorQ packets directly
        // This is more reliable than trying to extract from FSK-modulated audio
        let mut stream = encoder.encode_fountain(data, Some(config)).unwrap();

        let mut packet_serializations: Vec<Vec<u8>> = Vec::new();

        // Collect first few packets to analyze
        for _ in 0..20 {
            if let Some(packet) = stream.select_next_packet() {
                let serialized = packet.serialize();
                packet_serializations.push(serialized);
            } else {
                break;
            }
        }

        assert!(packet_serializations.len() >= 3, "Should collect at least 3 packet serializations (got {})", packet_serializations.len());

        // Count unique serialized packets
        let mut unique_packets = std::collections::HashSet::new();
        for pkt in &packet_serializations {
            unique_packets.insert(pkt.clone());
        }

        // With source packets + repair packets, we should see at least 2 unique patterns
        assert!(
            unique_packets.len() >= 2,
            "Should see at least 2 unique serialized packets (source + repair), got {}",
            unique_packets.len()
        );

        info!("Generated {} unique serialized packets from {} total", unique_packets.len(), packet_serializations.len());
    }

    #[test]
    fn test_fountain_source_packets_are_identical() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Source packet identity test";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 64,
            repair_blocks_ratio: 0.0, // Only source packets, no repairs
        };

        let mut stream = encoder.encode_fountain(data, Some(config)).unwrap();

        // With repair_ratio 0.0, select_next_packet should return source packets from the same set
        let mut source_packets: Vec<Vec<u8>> = Vec::new();

        for _ in 0..10 {
            if let Some(packet) = stream.select_next_packet() {
                source_packets.push(packet.serialize());
            } else {
                break;
            }
        }

        assert!(source_packets.len() >= 2, "Should get at least 2 source packets (got {})", source_packets.len());

        // With repair_ratio 0.0, it should cycle through source packets repeatedly
        // So the first few source packets should repeat
        let pkt1 = &source_packets[0];
        let pkt2 = &source_packets[1];

        // Check if we have the same packet (indicating cycling through source)
        // OR if they're different (indicating multiple source blocks)
        // Either is valid - just verify the mechanism works

        if source_packets.len() >= stream.source_packets.len() * 2 {
            // If we got at least 2 full cycles, first packet should repeat somewhere
            let found_repeat = source_packets.iter().skip(stream.source_packets.len())
                .any(|pkt| pkt == pkt1);
            assert!(found_repeat, "With repair_ratio=0, source packets should repeat in cycles");
        } else {
            // Not enough packets to verify cycling yet, just verify we got packets
            assert!(!source_packets.is_empty(), "Should have generated source packets");
        }

        info!("Generated {} source packets, {}unique serializations", source_packets.len(),
            source_packets.iter().collect::<std::collections::HashSet<_>>().len());
    }

    #[test]
    fn test_fountain_packets_differ_from_metadata() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Test that packet payloads are different, not just metadata";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 64,
            repair_blocks_ratio: 1.0,
        };

        let mut stream = encoder.encode_fountain(data, Some(config)).unwrap();

        // Collect serialized packets to directly compare their payloads
        let mut packets: Vec<Vec<u8>> = Vec::new();

        for _ in 0..30 {
            if let Some(packet) = stream.select_next_packet() {
                packets.push(packet.serialize());
            } else {
                break;
            }
        }

        assert!(packets.len() >= 5, "Should collect at least 5 packets (got {})", packets.len());

        // Count unique packet serializations
        let mut unique_packets = std::collections::HashSet::new();
        for pkt in &packets {
            unique_packets.insert(pkt.clone());
        }

        // Key assertion: With repair packets, we should see multiple unique serializations
        // This proves that blocks contain different data, not just different metadata
        //
        // - Source packets (K packets): may repeat in cycles
        // - Repair packets (ESI >= K): each counter value produces different repair packets
        //
        // So we should see at least: 1 unique source pattern + 2+ unique repair patterns = 3+ unique
        // Or at minimum 2 unique (source repeated, then at least 1 repair different)
        assert!(
            unique_packets.len() >= 2,
            "Should see at least 2 unique packet serializations with repair packets enabled. Got {}. This suggests packets might not have unique data payloads.",
            unique_packets.len()
        );

        // Count how many are likely repair packets (come after source packets)
        let num_source_expected = stream.source_packets.len();
        let num_repairs_generated = packets.len().saturating_sub(num_source_expected);

        info!(
            "Generated {} total packets: ~{} source, ~{} repairs. Found {} unique serializations",
            packets.len(), num_source_expected, num_repairs_generated, unique_packets.len()
        );
    }

    #[test]
    fn test_fountain_repair_counter_increments() {
        let mut encoder = EncoderFsk::new().unwrap();
        // Use longer data to ensure multiple source packets
        let data = b"Repair counter increment test with enough data to have multiple source blocks";

        let config = FountainConfig {
            timeout_secs: 5,
            block_size: 64,
            repair_blocks_ratio: 1.0,
        };

        let mut stream = encoder.encode_fountain(data, Some(config)).unwrap();

        // Verify repair_counters are initialized
        let initial_counters = stream.repair_counters.clone();
        assert!(!initial_counters.is_empty(), "Should have repair counters for source blocks");

        let num_source_packets = stream.source_packets.len();
        let num_blocks = stream.repair_counters.len();

        // Generate enough blocks to ensure we get into repair packet generation
        // With repair_ratio 1.0, we should get source_packets + repairs_per_cycle packets per cycle
        let block_count = num_source_packets * 2 + 20;

        for _ in 0..block_count {
            if stream.next().is_none() {
                break;
            }
        }

        // After generation, repair counters should have been incremented (assuming we generated repair packets)
        let final_counters = stream.repair_counters.clone();

        // If we have multiple blocks and repair ratio > 0, at least some counters should be > 0
        if num_blocks > 1 && stream.repairs_per_cycle > 0 {
            let incremented_count = final_counters.iter().filter(|c| **c > 0).count();
            assert!(
                incremented_count > 0,
                "With {} blocks and repair_ratio=1.0, at least one counter should be incremented, got: {:?}",
                num_blocks, final_counters
            );
        }

        info!(
            "Repair counters after generation: {:?} (num_blocks={}, repairs_per_cycle={})",
            final_counters, num_blocks, stream.repairs_per_cycle
        );
    }
}
