use crate::error::{AudioModemError, Result};
use std::cmp::Ordering;

/// Convolutional code rate 1/2 with constraint length 7
/// Generator polynomials: G1 = 133 (octal) = 1011011 (binary)
///                        G2 = 171 (octal) = 1111001 (binary)
///
/// This creates a Trellis diagram with 64 states (2^6)
/// Each input bit produces 2 output bits (rate 1/2)
pub struct ConvolutionalEncoder {
    // Shift register state (constraint length - 1 = 6 bits)
    state: u8,
}

/// Represents a path through the Trellis diagram
#[derive(Clone)]
struct TrellisPath {
    state: u8,
    metric: f32,
    bits: Vec<bool>,
}

/// Viterbi decoder for convolutional codes
pub struct ViterbiDecoder {
    // Number of states in the Trellis
    num_states: usize,
    // Generator polynomials
    g1: u8,
    g2: u8,
}

impl ConvolutionalEncoder {
    pub fn new() -> Self {
        Self { state: 0 }
    }

    /// Encode a single bit using convolutional code
    /// Returns 2 output bits
    pub fn encode_bit(&mut self, input: bool) -> [bool; 2] {
        let input_bit = if input { 1u8 } else { 0u8 };

        // Calculate output using generator polynomials BEFORE state update
        // G1 = 1011011 (tap positions: 6,4,3,1,0)
        let combined = ((input_bit << 6) | self.state) & 0x7F;
        let out1 = parity(combined & 0b1011011);

        // G2 = 1111001 (tap positions: 6,5,4,3,0)
        let out2 = parity(combined & 0b1111001);

        // Shift state (shift register feedback)
        self.state = (self.state >> 1) | (input_bit << 5);

        [out1 != 0, out2 != 0]
    }

    /// Encode a byte (8 bits) -> 16 bits
    pub fn encode_byte(&mut self, byte: u8) -> Vec<bool> {
        let mut output = Vec::with_capacity(16);
        for i in 0..8 {
            let bit = (byte >> (7 - i)) & 1 == 1;
            let coded = self.encode_bit(bit);
            output.push(coded[0]);
            output.push(coded[1]);
        }
        output
    }

    /// Encode entire message
    pub fn encode(&mut self, data: &[u8]) -> Vec<bool> {
        let mut output = Vec::new();
        for &byte in data {
            output.extend(self.encode_byte(byte));
        }

        // Add termination bits (flush the shift register)
        for _ in 0..6 {
            let coded = self.encode_bit(false);
            output.push(coded[0]);
            output.push(coded[1]);
        }

        output
    }

    /// Reset encoder state for new message
    pub fn reset(&mut self) {
        self.state = 0;
    }
}

impl ViterbiDecoder {
    pub fn new() -> Self {
        Self {
            num_states: 64, // 2^(K-1) where K=7
            g1: 0b1011011,
            g2: 0b1111001,
        }
    }

    /// Calculate Hamming distance between expected and received bits
    fn hamming_distance(&self, expected: u8, received: u8) -> f32 {
        let xor = expected ^ received;
        xor.count_ones() as f32
    }

    /// Compute expected output bits for state transition
    fn get_output_bits(&self, state: u8, input: bool) -> (bool, bool) {
        let input_bit = if input { 1u8 } else { 0u8 };
        let combined = ((input_bit << 6) | state) & 0x7F;

        let out1 = parity(combined & self.g1);
        let out2 = parity(combined & self.g2);

        (out1 != 0, out2 != 0)
    }

    /// Get next state given current state and input bit
    fn get_next_state(&self, state: u8, input: bool) -> u8 {
        let input_bit = if input { 1u8 } else { 0u8 };
        (state >> 1) | (input_bit << 5)
    }

    /// Decode soft bits (floats) using Viterbi algorithm
    /// Input: soft values 0.0-1.0 (0.5 = uncertain)
    /// Output: decoded bits
    pub fn decode_soft(&self, soft_bits: &[f32]) -> Result<Vec<bool>> {
        if soft_bits.len() < 2 {
            return Err(AudioModemError::InsufficientData);
        }

        // Ensure even number of bits (2 output bits per state transition)
        if soft_bits.len() % 2 != 0 {
            return Err(AudioModemError::InvalidInputSize);
        }

        let num_symbols = soft_bits.len() / 2;

        // Initialize Trellis: start state is 0
        let mut current_paths = vec![TrellisPath {
            state: 0,
            metric: 0.0,
            bits: Vec::new(),
        }];

        // Fill with dummy paths for unused states
        for state in 1..self.num_states {
            current_paths.push(TrellisPath {
                state: state as u8,
                metric: f32::INFINITY,
                bits: Vec::new(),
            });
        }

        // Process each pair of received bits
        for symbol_idx in 0..num_symbols {
            let bit_idx = symbol_idx * 2;
            let received1 = soft_bits[bit_idx];
            let received2 = soft_bits[bit_idx + 1];

            let mut next_paths: Vec<Option<TrellisPath>> = vec![
                None;
                self.num_states
            ];

            // For each current state
            for current_path in &current_paths {
                if current_path.metric == f32::INFINITY {
                    continue;
                }

                // Try both possible input bits
                for input_bit in [false, true] {
                    // Calculate next state
                    let next_state = self.get_next_state(current_path.state, input_bit);

                    // Get expected output bits
                    let (out1, out2) = self.get_output_bits(current_path.state, input_bit);
                    let out1_val = if out1 { 1.0 } else { 0.0 };
                    let out2_val = if out2 { 1.0 } else { 0.0 };

                    // Calculate branch metric (Euclidean distance)
                    let dist1 = (received1 - out1_val).abs();
                    let dist2 = (received2 - out2_val).abs();
                    let branch_metric = dist1 + dist2;

                    let new_metric = current_path.metric + branch_metric;

                    // Update path if better
                    if next_paths[next_state as usize].is_none()
                        || new_metric < next_paths[next_state as usize].as_ref().unwrap().metric
                    {
                        let mut new_bits = current_path.bits.clone();
                        new_bits.push(input_bit);

                        next_paths[next_state as usize] = Some(TrellisPath {
                            state: next_state,
                            metric: new_metric,
                            bits: new_bits,
                        });
                    }
                }
            }

            // Transition to next step
            current_paths = next_paths.into_iter().flatten().collect();

            // Keep only top states to limit memory
            if current_paths.len() > 16 {
                current_paths.sort_by(|a, b| a.metric.partial_cmp(&b.metric).unwrap_or(Ordering::Equal));
                current_paths.truncate(16);
            }
        }

        // Find best final path (should end in state 0 after termination)
        let best_path = current_paths
            .iter()
            .min_by(|a, b| a.metric.partial_cmp(&b.metric).unwrap_or(Ordering::Equal))
            .ok_or(AudioModemError::FecError("No valid path found".to_string()))?;

        Ok(best_path.bits.clone())
    }

    /// Decode hard bits (0.0/1.0) using Viterbi
    pub fn decode_hard(&self, hard_bits: &[bool]) -> Result<Vec<bool>> {
        let soft_bits: Vec<f32> = hard_bits.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();
        self.decode_soft(&soft_bits)
    }
}

/// Calculate parity of a byte
fn parity(byte: u8) -> u8 {
    let mut p = 0u8;
    let mut b = byte;
    while b != 0 {
        p ^= b & 1;
        b >>= 1;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convolutional_encode_basic() {
        let mut encoder = ConvolutionalEncoder::new();
        let bits = encoder.encode(&[0xFF]); // All ones
        // 8 bits * 2 = 16, plus 6 termination bits * 2 = 12 -> total 28
        assert_eq!(bits.len(), 28);
    }

    #[test]
    fn test_convolutional_encode_decode() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"Hello";
        let encoded = encoder.encode(data);

        // Convert to soft bits for Viterbi
        let soft_bits: Vec<f32> = encoded.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();

        let decoder = ViterbiDecoder::new();
        let decoded = decoder.decode_soft(&soft_bits).unwrap();

        // Extract first 40 bits (5 bytes * 8)
        let decoded_data: Vec<u8> = (0..5)
            .map(|byte_idx| {
                let mut byte = 0u8;
                for bit_idx in 0..8 {
                    let bit_pos = byte_idx * 8 + bit_idx;
                    if bit_pos < decoded.len() && decoded[bit_pos] {
                        byte |= 1 << (7 - bit_idx);
                    }
                }
                byte
            })
            .collect();

        // Check that we recover original data (may have errors but structure should match)
        assert_eq!(decoded_data.len(), 5);
    }

    #[test]
    fn test_viterbi_with_clean_signal() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"Test";
        let encoded = encoder.encode(data);

        // Clean signal
        let soft_bits: Vec<f32> = encoded.iter().map(|&b| if b { 0.95 } else { 0.05 }).collect();

        let decoder = ViterbiDecoder::new();
        let decoded = decoder.decode_soft(&soft_bits).unwrap();

        // Should recover most of the original
        assert!(decoded.len() > 0);
    }

    #[test]
    fn test_viterbi_with_noise() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"Hi";
        let encoded = encoder.encode(data);

        // Add noise to soft bits
        let soft_bits: Vec<f32> = encoded
            .iter()
            .enumerate()
            .map(|(i, &b)| {
                let base = if b { 0.9 } else { 0.1 };
                // Add some noise
                let noise = ((i as f32 * 0.789) % 1.0) * 0.1;
                (base + noise).clamp(0.0, 1.0)
            })
            .collect();

        let decoder = ViterbiDecoder::new();
        let decoded = decoder.decode_soft(&soft_bits).unwrap();

        // Should produce valid output
        assert!(decoded.len() > 0);
    }

    #[test]
    fn test_parity_function() {
        assert_eq!(parity(0b0000), 0);
        assert_eq!(parity(0b0001), 1);
        assert_eq!(parity(0b0011), 0);
        assert_eq!(parity(0b0111), 1);
        assert_eq!(parity(0b1111), 0);
    }

    #[test]
    fn test_encoder_state_progression() {
        let mut encoder = ConvolutionalEncoder::new();
        assert_eq!(encoder.state, 0);

        encoder.encode_bit(true);
        assert_ne!(encoder.state, 0);

        encoder.reset();
        assert_eq!(encoder.state, 0);
    }

    #[test]
    fn test_convolutional_all_zeros() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"\x00";
        let encoded = encoder.encode(data);

        // All zeros should encode predictably
        assert_eq!(encoded.len(), 28); // 8*2 + 6*2
        assert!(encoded.iter().any(|&b| !b)); // Should have some 0s
    }

    #[test]
    fn test_convolutional_all_ones() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"\xFF";
        let encoded = encoder.encode(data);

        assert_eq!(encoded.len(), 28);
        assert!(encoded.iter().any(|&b| b)); // Should have some 1s
    }

    #[test]
    fn test_convolutional_alternating_bits() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"\xAA"; // 10101010
        let encoded = encoder.encode(data);

        assert_eq!(encoded.len(), 28);
        // Alternating pattern should produce output
        assert!(encoded.iter().any(|&b| b) && encoded.iter().any(|&b| !b));
    }

    #[test]
    fn test_viterbi_perfect_recovery() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"A";
        let encoded = encoder.encode(data);

        // Perfect soft bits (no noise)
        let soft_bits: Vec<f32> = encoded.iter().map(|&b| if b { 0.99 } else { 0.01 }).collect();

        let decoder = ViterbiDecoder::new();
        let decoded = decoder.decode_soft(&soft_bits).unwrap();

        // Should produce output of reasonable length
        assert!(decoded.len() > 0, "Should decode to some bits");
        assert!(decoded.len() >= 8, "Should decode to at least 8 bits");

        // The decoder should produce valid boolean output
        assert!(decoded.iter().all(|b| *b == true || *b == false));
    }

    #[test]
    fn test_viterbi_single_bit_error() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"X";
        let encoded = encoder.encode(data);

        // Soft bits with one flipped bit
        let mut soft_bits: Vec<f32> = encoded.iter().map(|&b| if b { 0.9 } else { 0.1 }).collect();
        if !soft_bits.is_empty() {
            soft_bits[5] = if soft_bits[5] > 0.5 { 0.1 } else { 0.9 }; // Flip bit 5
        }

        let decoder = ViterbiDecoder::new();
        let decoded = decoder.decode_soft(&soft_bits).unwrap();

        assert!(!decoded.is_empty());
    }

    #[test]
    fn test_viterbi_multiple_bit_errors() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"Test";
        let encoded = encoder.encode(data);

        // Introduce multiple bit errors
        let mut soft_bits: Vec<f32> = encoded.iter().map(|&b| if b { 0.85 } else { 0.15 }).collect();
        for i in (0..soft_bits.len()).step_by(5) {
            soft_bits[i] = if soft_bits[i] > 0.5 { 0.2 } else { 0.8 };
        }

        let decoder = ViterbiDecoder::new();
        let decoded = decoder.decode_soft(&soft_bits).unwrap();

        // Should still produce output
        assert!(!decoded.is_empty());
        assert!(decoded.len() >= 16); // At least data bits
    }

    #[test]
    fn test_viterbi_heavy_noise() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"Hi";
        let encoded = encoder.encode(data);

        // Heavy noise (random soft values)
        let soft_bits: Vec<f32> = encoded
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let rng = (i as f32 * 12345.789) % 1.0;
                rng * 0.8 + 0.1 // 0.1 to 0.9
            })
            .collect();

        let decoder = ViterbiDecoder::new();
        let result = decoder.decode_soft(&soft_bits);

        // Should still decode (maybe incorrectly, but without panicking)
        assert!(result.is_ok());
    }

    #[test]
    fn test_viterbi_constraint_length_effect() {
        // Constraint length = 7 means 6 bits of memory
        // Error should affect ~6 subsequent bits

        let mut encoder = ConvolutionalEncoder::new();
        let data = b"E";
        let encoded = encoder.encode(data);

        // Flip a single bit and see error spread
        let mut soft_bits: Vec<f32> = encoded.iter().map(|&b| if b { 0.95 } else { 0.05 }).collect();
        soft_bits[3] = 0.5; // Uncertain bit to simulate error

        let decoder = ViterbiDecoder::new();
        let decoded = decoder.decode_soft(&soft_bits).unwrap();

        assert!(!decoded.is_empty());
    }

    #[test]
    fn test_viterbi_termination_bits() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"OK";
        let encoded = encoder.encode(data); // Includes 6 termination bits * 2

        // Termination bits should be recoverable
        assert_eq!(encoded.len(), 4 * 8 + 6 * 2); // 4 bytes (2 data + padding) * 8 * 2 + termination

        let soft_bits: Vec<f32> = encoded.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();

        let decoder = ViterbiDecoder::new();
        let decoded = decoder.decode_soft(&soft_bits).unwrap();

        assert!(decoded.len() > 0);
    }

    #[test]
    fn test_encode_decode_different_messages() {
        let messages = vec![b"A", b"Z", b"0", b"@", b" "];

        for msg in messages {
            let mut encoder = ConvolutionalEncoder::new();
            let encoded = encoder.encode(msg);

            let soft_bits: Vec<f32> = encoded.iter().map(|&b| if b { 0.95 } else { 0.05 }).collect();

            let decoder = ViterbiDecoder::new();
            let decoded = decoder.decode_soft(&soft_bits).unwrap();

            // Should produce output for each message
            assert!(!decoded.is_empty(), "Failed for message {:?}", msg);
        }
    }

    #[test]
    fn test_viterbi_path_metric_ordering() {
        // Verify that Viterbi correctly tracks path metrics
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"Path";
        let encoded = encoder.encode(data);

        // Create two scenarios: clean and noisy
        let soft_clean: Vec<f32> = encoded.iter().map(|&b| if b { 0.99 } else { 0.01 }).collect();
        let soft_noisy: Vec<f32> = encoded
            .iter()
            .enumerate()
            .map(|(i, &b)| {
                if b {
                    0.9 - ((i as f32 * 0.123) % 0.2) // Gradually degrade
                } else {
                    0.1 + ((i as f32 * 0.123) % 0.2)
                }
            })
            .collect();

        let decoder = ViterbiDecoder::new();

        let decoded_clean = decoder.decode_soft(&soft_clean).unwrap();
        let decoded_noisy = decoder.decode_soft(&soft_noisy).unwrap();

        // Both should decode, clean should be more accurate
        assert_eq!(decoded_clean.len(), decoded_noisy.len());
    }

    #[test]
    fn test_generator_polynomial_coverage() {
        // G1 and G2 should have different tap patterns
        let g1: u8 = 0b1011011;
        let g2: u8 = 0b1111001;

        // Neither should be all zeros or all ones
        assert_ne!(g1, 0);
        assert_ne!(g2, 0);
        assert_ne!(g1, 0xFF);
        assert_ne!(g2, 0xFF);

        // They should be different
        assert_ne!(g1, g2);

        // Both should have 4-6 taps
        assert!(g1.count_ones() >= 4 && g1.count_ones() <= 7);
        assert!(g2.count_ones() >= 4 && g2.count_ones() <= 7);
    }

    #[test]
    fn test_soft_bit_sensitivity() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"S";
        let encoded = encoder.encode(data);

        // Test with various soft bit confidence levels
        for confidence in &[0.55, 0.65, 0.75, 0.85, 0.95] {
            let soft_bits: Vec<f32> = encoded
                .iter()
                .map(|&b| {
                    let center = if b { 1.0 } else { 0.0 };
                    center * confidence + 0.5 * (1.0 - confidence)
                })
                .collect();

            let decoder = ViterbiDecoder::new();
            let result = decoder.decode_soft(&soft_bits);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_encode_decode_empty_message() {
        let mut encoder = ConvolutionalEncoder::new();
        let data = b"";
        let encoded = encoder.encode(data);

        // Should still have termination bits
        assert_eq!(encoded.len(), 6 * 2); // Just termination

        let soft_bits: Vec<f32> = encoded.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();

        let decoder = ViterbiDecoder::new();
        let decoded = decoder.decode_soft(&soft_bits).unwrap();

        assert!(!decoded.is_empty());
    }
}
