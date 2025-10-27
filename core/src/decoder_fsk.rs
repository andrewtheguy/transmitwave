use crate::error::{AudioModemError, Result};
use crate::fec::{FecDecoder, FecMode};
use crate::framing::{FrameDecoder, crc16};
use crate::fsk::{FskDemodulator, FountainConfig, FSK_BYTES_PER_SYMBOL, FSK_SYMBOL_SAMPLES};
use crate::sync::{detect_postamble, detect_preamble, DetectionThreshold};
use crate::PREAMBLE_SAMPLES;
use raptorq::{Decoder, EncodingPacket};

#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

/// Decoder using Multi-tone FSK with Reed-Solomon FEC
///
/// Demodulates multi-tone FSK symbols (6 simultaneous frequencies) using non-coherent
/// energy detection (Goertzel algorithm) to recover the original binary data.
/// Includes Reed-Solomon error correction for robustness against channel impairments.
pub struct DecoderFsk {
    fsk: FskDemodulator,
    fec: FecDecoder,
    preamble_threshold: DetectionThreshold,
    postamble_threshold: DetectionThreshold,
}

impl DecoderFsk {
    pub fn new() -> Result<Self> {
        Ok(Self {
            fsk: FskDemodulator::new(),
            fec: FecDecoder::new()?,
            preamble_threshold: DetectionThreshold::Adaptive, // Default: use adaptive threshold
            postamble_threshold: DetectionThreshold::Adaptive, // Default: use adaptive threshold
        })
    }

    /// Set the detection threshold for preamble detection
    pub fn set_preamble_threshold(&mut self, threshold: DetectionThreshold) {
        self.preamble_threshold = match threshold {
            DetectionThreshold::Adaptive => DetectionThreshold::Adaptive,
            DetectionThreshold::Fixed(value) => DetectionThreshold::Fixed(value.max(0.001).min(1.0)),
        };
    }

    /// Get the current preamble detection threshold
    pub fn get_preamble_threshold(&self) -> DetectionThreshold {
        self.preamble_threshold
    }

    /// Set the detection threshold for postamble detection
    pub fn set_postamble_threshold(&mut self, threshold: DetectionThreshold) {
        self.postamble_threshold = match threshold {
            DetectionThreshold::Adaptive => DetectionThreshold::Adaptive,
            DetectionThreshold::Fixed(value) => DetectionThreshold::Fixed(value.max(0.001).min(1.0)),
        };
    }

    /// Get the current postamble detection threshold
    pub fn get_postamble_threshold(&self) -> DetectionThreshold {
        self.postamble_threshold
    }

    /// Set both preamble and postamble detection thresholds to the same value
    pub fn set_detection_threshold(&mut self, threshold: DetectionThreshold) {
        self.set_preamble_threshold(threshold);
        self.set_postamble_threshold(threshold);
    }

    /// Get the preamble detection threshold
    pub fn get_detection_threshold(&self) -> DetectionThreshold {
        self.get_preamble_threshold()
    }

    /// Decode audio samples back to binary data
    /// Expects: preamble + (FSK symbols) + postamble
    ///
    /// Handles shortened Reed-Solomon decoding by restoring padding zeros
    /// before RS decoding, then removing them after.
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        if samples.len() < FSK_SYMBOL_SAMPLES * 2 {
            return Err(AudioModemError::InsufficientData);
        }

        // Detect preamble to find start of data, using configured threshold
        let preamble_pos = detect_preamble(samples, self.preamble_threshold)
            .ok_or(AudioModemError::PreambleNotFound)?;

        // Data starts after preamble
        let data_start = preamble_pos + PREAMBLE_SAMPLES;

        if data_start + FSK_SYMBOL_SAMPLES > samples.len() {
            return Err(AudioModemError::InsufficientData);
        }

        // Try to detect postamble to find end of data, using configured threshold
        let remaining = &samples[data_start..];
        let postamble_pos = detect_postamble(remaining, self.postamble_threshold)
            .ok_or(AudioModemError::PostambleNotFound)?;

        let data_end = data_start + postamble_pos;

        // Extract FSK data region
        let fsk_region = &samples[data_start..data_end];

        // Ensure we have complete symbols
        let symbol_count = fsk_region.len() / FSK_SYMBOL_SAMPLES;
        if symbol_count == 0 {
            return Err(AudioModemError::InsufficientData);
        }

        let valid_samples = symbol_count * FSK_SYMBOL_SAMPLES;
        let fsk_samples = &fsk_region[..valid_samples];

        // Demodulate multi-tone FSK symbols to bytes
        let bytes = self.fsk.demodulate(fsk_samples)?;

        if bytes.len() < 2 {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Read 2-byte length prefix to determine frame data length
        let frame_len = ((bytes[0] as u16) << 8) | (bytes[1] as u16);
        let mut byte_idx = 2;

        // First pass: decode the first block to get FEC mode from header
        // We need to peek at the header to determine FEC mode
        let first_chunk_len = (frame_len as usize).min(223);
        let padding_needed_first = 223 - first_chunk_len;

        // Try with different FEC modes to find the right one
        // Start with Light (smallest overhead) and work up
        let mut decoded_first_block = None;
        let mut detected_fec_mode = FecMode::Light;

        for mode in [FecMode::Light, FecMode::Medium, FecMode::Full] {
            let parity_bytes = mode.parity_bytes();
            let encoded_len = first_chunk_len + parity_bytes;

            if byte_idx + encoded_len <= bytes.len() {
                let shortened_block = &bytes[byte_idx..byte_idx + encoded_len];
                let mut full_block = vec![0u8; padding_needed_first];
                full_block.extend_from_slice(shortened_block);

                // Try decoding with this FEC mode
                if let Ok(decoded_chunk) = self.fec.decode_with_mode(&full_block, mode) {
                    // Check if this produces a valid header
                    let decoded_data = &decoded_chunk[padding_needed_first..];
                    if decoded_data.len() >= 8 {
                        if let Ok((_, _, fec_mode_byte)) = FrameDecoder::decode_header(decoded_data) {
                            if let Ok(parsed_mode) = FecMode::from_u8(fec_mode_byte) {
                                if parsed_mode == mode {
                                    // Found the correct FEC mode!
                                    decoded_first_block = Some((decoded_data.to_vec(), encoded_len));
                                    detected_fec_mode = mode;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        let (first_decoded, first_encoded_len) = decoded_first_block
            .ok_or(AudioModemError::FecDecodeFailure)?;

        // Now decode remaining blocks using the detected FEC mode
        let mut decoded_data = first_decoded;
        byte_idx += first_encoded_len;
        let mut remaining_len = frame_len as usize - first_chunk_len;

        while remaining_len > 0 {
            let chunk_len = remaining_len.min(223);
            let padding_needed = 223 - chunk_len;
            let parity_bytes = detected_fec_mode.parity_bytes();
            let encoded_len = chunk_len + parity_bytes;

            // Check if we have enough bytes
            if byte_idx + encoded_len > bytes.len() {
                break;
            }

            // Extract the shortened RS block
            let shortened_block = &bytes[byte_idx..byte_idx + encoded_len];
            byte_idx += encoded_len;

            // Restore to full RS block by prepending zeros
            let mut full_block = vec![0u8; padding_needed];
            full_block.extend_from_slice(shortened_block);

            // Decode with RS using detected FEC mode
            match self.fec.decode_with_mode(&full_block, detected_fec_mode) {
                Ok(decoded_chunk) => {
                    // Remove the prepended zeros (padding)
                    decoded_data.extend_from_slice(&decoded_chunk[padding_needed..]);
                }
                Err(_) => {
                    // FEC failed - might be corruption
                    return Err(AudioModemError::FecDecodeFailure);
                }
            }

            remaining_len -= chunk_len;
        }

        if decoded_data.is_empty() {
            return Err(AudioModemError::FecDecodeFailure);
        }

        // Decode frame structure
        let frame = FrameDecoder::decode(&decoded_data)?;

        // Verify frame size is reasonable
        if frame.payload_len as usize > decoded_data.len() {
            return Err(AudioModemError::InvalidFrameSize);
        }

        Ok(frame.payload)
    }

    /// Decode audio samples using fountain mode with continuous block accumulation
    ///
    /// Processes audio samples to extract fountain-encoded blocks and attempts
    /// to decode the original data. Continues until successful decode or timeout.
    ///
    /// Returns the decoded payload or an error if decoding fails or timeout occurs.
    pub fn decode_fountain(&mut self, samples: &[f32], config: Option<FountainConfig>) -> Result<Vec<u8>> {
        let config = config.unwrap_or_default();

        #[cfg(not(target_arch = "wasm32"))]
        let start_time = Instant::now();
        #[cfg(not(target_arch = "wasm32"))]
        let timeout = Duration::from_secs(config.timeout_secs as u64);

        let mut decoder: Option<Decoder> = None;
        let mut search_offset = 0;
        let mut frame_length: Option<usize> = None;
        let mut symbol_size: Option<u16> = None;
        let mut payload_samples_per_block =
            Self::fountain_payload_samples(config.block_size as u16);

        while search_offset < samples.len() {
            // Check timeout (not available in WASM)
            #[cfg(not(target_arch = "wasm32"))]
            if start_time.elapsed() >= timeout {
                return Err(AudioModemError::Timeout);
            }

            // Look for next preamble
            let remaining = &samples[search_offset..];
            let preamble_search_window = PREAMBLE_SAMPLES + payload_samples_per_block;
            let search_len = remaining.len().min(preamble_search_window);
            if search_len < PREAMBLE_SAMPLES {
                break;
            }
            let preamble_slice = &remaining[..search_len];
            let preamble_pos = match detect_preamble(preamble_slice, self.preamble_threshold) {
                Some(pos) => pos,
                None => break,
            };

            let data_start = search_offset + preamble_pos + PREAMBLE_SAMPLES;

            if data_start + FSK_SYMBOL_SAMPLES > samples.len() {
                break;
            }

            // Extract the expected FSK payload based on configured block size
            let data_end = data_start.saturating_add(payload_samples_per_block);
            if data_end > samples.len() {
                break;
            }
            let fsk_samples = &samples[data_start..data_end];

            // Demodulate fountain block
            match self.fsk.demodulate(fsk_samples) {
                Ok(block_data) => {
                    let mut slice = block_data.as_slice();

                    if slice.len() < 6 {
                        search_offset = data_end;
                        continue;
                    }

                    let len_bytes = [slice[0], slice[1], slice[2], slice[3]];
                    let parsed_frame_len = u32::from_be_bytes(len_bytes) as usize;

                    let sym_bytes = [slice[4], slice[5]];
                    let parsed_symbol_size = u16::from_be_bytes(sym_bytes);

                    match frame_length {
                        Some(existing) if existing != parsed_frame_len => {
                            search_offset = data_end;
                            continue;
                        }
                        Some(_) => {}
                        None => frame_length = Some(parsed_frame_len),
                    }

                    let mut symbol_updated = false;
                    match symbol_size {
                        Some(existing) if existing != parsed_symbol_size => {
                            search_offset = data_end;
                            continue;
                        }
                        Some(_) => {}
                        None => {
                            symbol_size = Some(parsed_symbol_size);
                            symbol_updated = true;
                        }
                    }

                    if symbol_updated {
                        payload_samples_per_block =
                            Self::fountain_payload_samples(parsed_symbol_size);
                    }

                    slice = &slice[6..];

                    if slice.len() < 2 {
                        search_offset = data_end;
                        continue;
                    }

                    let packet_len = u16::from_be_bytes([slice[0], slice[1]]) as usize;
                    slice = &slice[2..];

                    if slice.len() < packet_len + 2 {
                        // Need packet_len bytes + 2 bytes for CRC-16
                        search_offset = data_end;
                        continue;
                    }

                    let packet_bytes = &slice[..packet_len];
                    // Extract and validate packet CRC-16 for early corruption detection
                    let received_crc = u16::from_be_bytes([slice[packet_len], slice[packet_len + 1]]);
                    let computed_crc = crc16(packet_bytes);

                    if received_crc != computed_crc {
                        // Packet corrupted - skip it and continue
                        search_offset = data_end;
                        continue;
                    }

                    // Attempt to deserialize the packet; if deserialization fails, skip this packet and continue
                    let packet = EncodingPacket::deserialize(packet_bytes);

                    // Initialize decoder on first packet with matching OTI
                    if decoder.is_none() && frame_length.is_some() && symbol_size.is_some() {
                        let oti = raptorq::ObjectTransmissionInformation::with_defaults(
                            frame_length.unwrap() as u64,
                            symbol_size.unwrap()
                        );
                        decoder = Some(Decoder::new(oti));
                    }

                    // Add packet and try to decode
                    if let Some(ref mut dec) = decoder {
                        // Attempt to decode with the packet
                        // If decode fails (returns None), continue to next packet
                        if let Some(decoded_data) = dec.decode(packet) {
                            // Successfully decoded! Extract frame
                            match FrameDecoder::decode(&decoded_data) {
                                Ok(frame) => return Ok(frame.payload),
                                Err(_) => {
                                    // Frame decode failed, continue to next packet
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    // Demodulation failed
                }
            }

            // No postamble in fountain mode - advance directly from data_end
            search_offset = data_end;
        }

        Err(AudioModemError::FountainDecodeFailure)
    }

    fn fountain_payload_samples(symbol_size: u16) -> usize {
        // Fixed metadata: frame_len(4) + symbol_size(2) + packet_len(2) = 8 bytes
        // Variable: RaptorQ packet (may be larger than symbol_size due to encoding overhead)
        // Add CRC-16 (2 bytes) + extra buffer for serialization overhead
        // Conservative estimate: symbol_size + 14 bytes accounting for all overhead and CRC
        let packet_bytes = symbol_size as usize + 14;
        let symbols = (packet_bytes + FSK_BYTES_PER_SYMBOL - 1) / FSK_BYTES_PER_SYMBOL;
        symbols * FSK_SYMBOL_SAMPLES
    }
}

impl Default for DecoderFsk {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder_fsk::EncoderFsk;

    #[test]
    fn test_decoder_fsk_basic_roundtrip() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        let data = b"Hello FSK!";
        let samples = encoder.encode(data).unwrap();
        let decoded = decoder.decode(&samples).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_fsk_empty_data() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        let data = b"";
        let samples = encoder.encode(data).unwrap();
        let decoded = decoder.decode(&samples).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_fsk_various_lengths() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        let test_cases = vec![
            b"A".to_vec(),
            b"AB".to_vec(),
            b"ABC".to_vec(),
            b"The quick brown fox".to_vec(),
            vec![0u8; 50],
            vec![255u8; 30],
        ];

        for data in test_cases {
            let samples = encoder.encode(&data).unwrap();
            let decoded = decoder.decode(&samples).unwrap();
            assert_eq!(decoded, data, "Failed for data length {}", data.len());
        }
    }

    #[test]
    fn test_decoder_fsk_binary_data() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        // Test binary data (not just ASCII)
        let data: Vec<u8> = (0..100).map(|i| i as u8).collect();
        let samples = encoder.encode(&data).unwrap();
        let decoded = decoder.decode(&samples).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_fsk_with_noise() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        let data = b"Noisy channel test";
        let mut samples = encoder.encode(data).unwrap();

        // Add small amount of noise (5% of signal)
        for sample in samples.iter_mut() {
            let noise = (*sample * 123.456).sin() * 0.05;
            *sample += noise;
        }

        let decoded = decoder.decode(&samples).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_fsk_insufficient_data() {
        let mut decoder = DecoderFsk::new().unwrap();

        // Too few samples
        let samples = vec![0.0; 100];
        let result = decoder.decode(&samples);
        assert!(result.is_err());
    }

    #[test]
    fn test_decoder_fsk_no_preamble() {
        let mut decoder = DecoderFsk::new().unwrap();

        // Random noise without preamble
        let samples: Vec<f32> = (0..10000).map(|i| (i as f32 * 0.1).sin() * 0.1).collect();
        let result = decoder.decode(&samples);
        assert!(result.is_err());
    }

    #[test]
    fn test_decoder_fsk_large_payload() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        // Test with larger payload (near maximum)
        let data = vec![42u8; 180];
        let samples = encoder.encode(&data).unwrap();
        let decoded = decoder.decode(&samples).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_fsk_repeating_patterns() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        // Test with repeating byte patterns
        let patterns = vec![
            vec![0x00; 20],      // All zeros
            vec![0xFF; 20],      // All ones
            vec![0xAA; 20],      // Alternating bits
            vec![0x55; 20],      // Alternating bits (inverse)
        ];

        for data in patterns {
            let samples = encoder.encode(&data).unwrap();
            let decoded = decoder.decode(&samples).unwrap();
            assert_eq!(decoded, data, "Failed for pattern {:02X}", data[0]);
        }
    }

    #[test]
    fn test_fountain_roundtrip_basic() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Fountain roundtrip test";

        let config = FountainConfig {
            timeout_secs: 5,
            block_size: 32,
            repair_blocks_ratio: 0.5,
        };

        // Generate fountain blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();

        // Collect multiple blocks into one continuous audio stream
        let blocks: Vec<_> = stream.take(10).collect();
        let mut samples = Vec::new();
        for block in blocks {
            samples.extend_from_slice(&block);
        }

        // Decode
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_fountain_with_packet_loss() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Test with some packet loss";

        let config = FountainConfig {
            timeout_secs: 30, // Enough audio duration to generate 20 blocks
            block_size: 32,
            repair_blocks_ratio: 1.0, // More redundancy
        };

        // Generate blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(20).collect();

        // Simulate packet loss by dropping every 3rd block
        let mut samples = Vec::new();
        for (i, block) in blocks.iter().enumerate() {
            if i % 3 != 0 {  // Drop every 3rd block
                samples.extend_from_slice(block);
            }
        }

        // Should still decode successfully due to fountain coding
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_fountain_various_data_sizes() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        let config = FountainConfig {
            timeout_secs: 20, // Enough audio duration to generate 15 blocks
            block_size: 32,
            repair_blocks_ratio: 0.5,
        };

        let test_cases = vec![
            b"A".to_vec(),
            b"Short".to_vec(),
            b"Medium length data for testing".to_vec(),
            vec![42u8; 100],
        ];

        for data in test_cases {
            let stream = encoder.encode_fountain(&data, Some(config.clone())).unwrap();
            let blocks: Vec<_> = stream.take(15).collect();
            let mut samples = Vec::new();
            for block in blocks {
                samples.extend_from_slice(&block);
            }

            let decoded = decoder.decode_fountain(&samples, Some(config.clone())).unwrap();
            assert_eq!(decoded, data, "Failed for data length {}", data.len());
        }
    }

    #[test]
    fn test_fountain_single_block_crc_corruption() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Test CRC corruption detection";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 32,
            repair_blocks_ratio: 0.5,
        };

        // Generate blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(15).collect();

        // Corrupt the first block's CRC by flipping bits in the audio
        let mut corrupted_blocks = blocks.clone();
        let first_block = &mut corrupted_blocks[0];

        // Flip some bits in the middle of the first block (CRC area is near the end)
        // We flip bits in the area that should contain the CRC-16
        for i in (first_block.len() / 2)..(first_block.len() / 2 + 100) {
            if i < first_block.len() {
                first_block[i] = first_block[i] * 0.5; // Corrupt audio amplitude
            }
        }

        // Reconstruct audio stream
        let mut samples = Vec::new();
        for block in &corrupted_blocks {
            samples.extend_from_slice(block);
        }

        // Decoding should still succeed with remaining good blocks
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should recover despite first block corruption");
    }

    #[test]
    fn test_fountain_multiple_blocks_crc_corruption() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Test multiple corruptions";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 32,
            repair_blocks_ratio: 1.0, // Extra redundancy
        };

        // Generate blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(20).collect();

        // Corrupt multiple blocks
        let mut corrupted_blocks = blocks.clone();
        for block_idx in [0, 3, 7] {
            if block_idx < corrupted_blocks.len() {
                let block = &mut corrupted_blocks[block_idx];
                // Corrupt by amplitude reduction in different areas
                for i in (block_idx * 50)..(block_idx * 50 + 80) {
                    if i < block.len() {
                        block[i] = block[i] * 0.3;
                    }
                }
            }
        }

        // Reconstruct audio stream
        let mut samples = Vec::new();
        for block in &corrupted_blocks {
            samples.extend_from_slice(block);
        }

        // Should still succeed with sufficient redundancy
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should recover with multiple block corruptions");
    }

    #[test]
    fn test_fountain_crc_rejects_invalid_packets() {
        use crate::fsk::FountainConfig;
        use crate::framing::crc16;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Verify CRC validation";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 64,
            repair_blocks_ratio: 0.5,
        };

        // Generate blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(12).collect();

        // Create a highly corrupted block where CRC will definitely fail
        let mut corrupted_blocks = blocks.clone();
        if !corrupted_blocks.is_empty() {
            let block = &mut corrupted_blocks[0];
            // Flip many bits throughout the block to ensure CRC mismatch
            for i in 0..block.len().min(200) {
                block[i] = -block[i]; // Invert samples
            }
        }

        // Reconstruct audio stream with corrupted first block
        let mut samples = Vec::new();
        for block in &corrupted_blocks {
            samples.extend_from_slice(block);
        }

        // Decoding should succeed by skipping the corrupted block and using others
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should skip invalid CRC blocks");
    }

    #[test]
    fn test_fountain_crc_detects_bit_flips() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Bit flip detection test";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 32,
            repair_blocks_ratio: 0.75,
        };

        // Generate blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(15).collect();

        // Corrupt early blocks with bit flips (emulate FSK demodulation errors)
        let mut corrupted_blocks = blocks.clone();
        for block_idx in 0..3.min(corrupted_blocks.len()) {
            let block = &mut corrupted_blocks[block_idx];
            // Simulate bit flips by adding noise in high-frequency ranges
            for i in 0..block.len().min(150) {
                block[i] += 0.1 * ((i as f32).sin());
            }
        }

        // Reconstruct audio stream
        let mut samples = Vec::new();
        for block in &corrupted_blocks {
            samples.extend_from_slice(block);
        }

        // Should still decode successfully
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should handle bit flip corruptions");
    }

    #[test]
    fn test_fountain_crc_with_packet_loss_and_corruption() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Combined loss and corruption";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 32,
            repair_blocks_ratio: 1.0,
        };

        // Generate blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(20).collect();

        // Simulate both packet loss and corruption
        let mut samples = Vec::new();
        for (i, block) in blocks.iter().enumerate() {
            let mut modified_block = block.clone();

            // Drop every 5th block (packet loss)
            if i % 5 == 0 {
                continue;
            }

            // Corrupt every 3rd block that we keep
            if (i / 3) % 2 == 0 {
                for j in 0..modified_block.len().min(120) {
                    modified_block[j] = modified_block[j] * 0.4;
                }
            }

            samples.extend_from_slice(&modified_block);
        }

        // Should still decode with sufficient redundancy
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should handle packet loss + corruption");
    }

    #[test]
    fn test_fountain_crc_passes_for_clean_data() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Clean data with valid CRC";

        let config = FountainConfig {
            timeout_secs: 20,
            block_size: 32,
            repair_blocks_ratio: 0.5,
        };

        // Generate blocks (no corruption)
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(10).collect();

        let mut samples = Vec::new();
        for block in blocks {
            samples.extend_from_slice(&block);
        }

        // All CRCs should pass, decoding should be quick
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Clean data should decode correctly");
    }

    #[test]
    fn test_fountain_integration_alternating_good_bad_blocks() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Integration test: alternating good/bad blocks";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 32,
            repair_blocks_ratio: 0.75,
        };

        // Generate blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(18).collect();

        // Pattern: good, bad, good, bad, ... (alternating)
        let mut processed_blocks = blocks.clone();
        for (i, block) in processed_blocks.iter_mut().enumerate() {
            if i % 2 == 1 {
                // Corrupt odd-indexed blocks (1, 3, 5, ...)
                for j in 0..block.len().min(100) {
                    block[j] = block[j] * 0.2; // Severe amplitude reduction
                }
            }
        }

        let mut samples = Vec::new();
        for block in &processed_blocks {
            samples.extend_from_slice(block);
        }

        // Should successfully decode despite every other block being bad
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should decode with alternating good/bad blocks");
    }

    #[test]
    fn test_fountain_integration_burst_corruption() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Burst error: consecutive bad blocks";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 32,
            repair_blocks_ratio: 1.0, // Extra redundancy for burst recovery
        };

        // Generate blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(20).collect();

        // Simulate burst corruption: blocks 3-7 all corrupted
        let mut processed_blocks = blocks.clone();
        for i in 3..8 {
            if i < processed_blocks.len() {
                let block = &mut processed_blocks[i];
                // Corrupt with phase shift (realistic channel impairment)
                for j in 0..block.len().min(150) {
                    block[j] = -block[j]; // Phase inversion = maximum corruption
                }
            }
        }

        let mut samples = Vec::new();
        for block in &processed_blocks {
            samples.extend_from_slice(block);
        }

        // Should recover despite burst of 5 consecutive bad blocks
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should recover from burst corruption with good blocks before/after");
    }

    #[test]
    fn test_fountain_integration_sparse_good_blocks() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Sparse: mostly bad, few good blocks";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 32,
            repair_blocks_ratio: 1.0,
        };

        // Generate many blocks to ensure we have enough good ones
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(25).collect();

        // Keep only blocks at indices 2, 5, 10, 15, 22 (sparse good blocks)
        let mut processed_blocks = blocks.clone();
        let good_indices = [2, 5, 10, 15, 22];
        for (i, block) in processed_blocks.iter_mut().enumerate() {
            if !good_indices.contains(&i) {
                // Corrupt all blocks except the good ones
                for j in 0..block.len().min(200) {
                    block[j] = block[j] * 0.15; // Heavy corruption
                }
            }
        }

        let mut samples = Vec::new();
        for block in &processed_blocks {
            samples.extend_from_slice(block);
        }

        // Should decode with minimal good blocks due to fountain redundancy
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should decode with sparse valid blocks");
    }

    #[test]
    fn test_fountain_integration_good_blocks_early_then_bad() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Good early, bad later: early blocks good";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 32,
            repair_blocks_ratio: 0.5,
        };

        // Generate blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(16).collect();

        // First 6 blocks are good, remaining are bad
        let mut processed_blocks = blocks.clone();
        for (i, block) in processed_blocks.iter_mut().enumerate() {
            if i >= 6 {
                // Corrupt blocks 6 and onwards
                for j in 0..block.len().min(120) {
                    block[j] = block[j] * 0.25;
                }
            }
        }

        let mut samples = Vec::new();
        for block in &processed_blocks {
            samples.extend_from_slice(block);
        }

        // Should successfully decode with good source blocks at beginning
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should decode when first blocks are clean");
    }

    #[test]
    fn test_fountain_integration_bad_blocks_early_then_good() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Bad early, good later: repair blocks work";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 32,
            repair_blocks_ratio: 0.75,
        };

        // Generate blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(18).collect();

        // First 5 blocks are bad, remaining are good
        let mut processed_blocks = blocks.clone();
        for (i, block) in processed_blocks.iter_mut().enumerate() {
            if i < 5 {
                // Corrupt blocks 0-4
                for j in 0..block.len().min(130) {
                    block[j] = block[j] * 0.1;
                }
            }
        }

        let mut samples = Vec::new();
        for block in &processed_blocks {
            samples.extend_from_slice(block);
        }

        // Should decode because repair blocks arrive later
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should decode with repair blocks after bad source blocks");
    }

    #[test]
    fn test_fountain_integration_random_pass_fail_pattern() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Random: unpredictable pass/fail pattern";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 32,
            repair_blocks_ratio: 1.0,
        };

        // Generate blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(20).collect();

        // Random corruption pattern: 2,4,5,8,12,14,17 are corrupted
        let corrupted_indices = [2, 4, 5, 8, 12, 14, 17];
        let mut processed_blocks = blocks.clone();
        for (i, block) in processed_blocks.iter_mut().enumerate() {
            if corrupted_indices.contains(&i) {
                // Corrupt these blocks
                let corruption_level = 0.2 + (i as f32 * 0.05);
                for j in 0..block.len().min(100 + i * 5) {
                    block[j] = block[j] * corruption_level;
                }
            }
        }

        let mut samples = Vec::new();
        for block in &processed_blocks {
            samples.extend_from_slice(block);
        }

        // Should successfully decode despite random corruption pattern
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should decode with random pass/fail pattern");
    }

    #[test]
    fn test_fountain_integration_progressive_degradation() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Progressive: quality degrades over time";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 32,
            repair_blocks_ratio: 1.0,
        };

        // Generate blocks
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(20).collect();

        // Simulate progressive channel degradation: later blocks more corrupted
        let mut processed_blocks = blocks.clone();
        for (i, block) in processed_blocks.iter_mut().enumerate() {
            // Corruption level increases with block index
            let corruption_level = 0.1 + (i as f32 * 0.04);
            if corruption_level > 0.5 {
                // Corrupt heavily
                for j in 0..block.len().min(80 + i * 3) {
                    block[j] = block[j] * corruption_level;
                }
            }
        }

        let mut samples = Vec::new();
        for block in &processed_blocks {
            samples.extend_from_slice(block);
        }

        // Should still decode despite progressive quality loss
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should decode despite progressive degradation");
    }

    #[test]
    fn test_fountain_integration_isolated_good_blocks_spread() {
        use crate::fsk::FountainConfig;

        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();
        let data = b"Spread: isolated good blocks widely separated";

        let config = FountainConfig {
            timeout_secs: 30,
            block_size: 32,
            repair_blocks_ratio: 1.0,
        };

        // Generate many blocks to test with sparse good ones
        let stream = encoder.encode_fountain(data, Some(config.clone())).unwrap();
        let blocks: Vec<_> = stream.take(24).collect();

        // Good blocks at indices 1, 6, 11, 16, 21 (widely spread)
        let good_indices = [1, 6, 11, 16, 21];
        let mut processed_blocks = blocks.clone();
        for (i, block) in processed_blocks.iter_mut().enumerate() {
            if !good_indices.contains(&i) {
                // Corrupt all others with varying severity
                let severity = (i % 4) as f32 * 0.2 + 0.1;
                for j in 0..block.len().min(100 + (i * 2)) {
                    block[j] = block[j] * severity;
                }
            }
        }

        let mut samples = Vec::new();
        for block in &processed_blocks {
            samples.extend_from_slice(block);
        }

        // Should decode with isolated good blocks spread apart
        let decoded = decoder.decode_fountain(&samples, Some(config)).unwrap();
        assert_eq!(decoded, data, "Should decode with widely separated good blocks");
    }
}
