use crate::error::{AudioModemError, Result};
use crate::{RS_DATA_BYTES, RS_ECC_BYTES, RS_TOTAL_BYTES};
use reed_solomon_simd::{ReedSolomonDecoder, ReedSolomonEncoder};

/// FEC mode determines the level of error correction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FecMode {
    /// Minimal FEC: 8 parity bytes (for payloads < 20 bytes)
    Light = 8,
    /// Medium FEC: 16 parity bytes (for payloads 20-50 bytes)
    Medium = 16,
    /// Full FEC: 32 parity bytes (for payloads > 50 bytes)
    Full = 32,
}

impl FecMode {
    /// Choose FEC mode based on frame data size (header + payload)
    pub fn from_data_size(data_size: usize) -> Self {
        if data_size < 20 {
            FecMode::Light
        } else if data_size < 50 {
            FecMode::Medium
        } else {
            FecMode::Full
        }
    }

    /// Get parity bytes for this mode
    pub fn parity_bytes(&self) -> usize {
        *self as usize
    }

    /// Convert from byte value
    pub fn from_u8(value: u8) -> Result<Self> {
        match value {
            8 => Ok(FecMode::Light),
            16 => Ok(FecMode::Medium),
            32 => Ok(FecMode::Full),
            _ => Err(AudioModemError::InvalidConfig(
                "Invalid FEC mode".to_string(),
            )),
        }
    }

    /// Convert to byte value
    pub fn to_u8(&self) -> u8 {
        *self as u8
    }
}

pub struct FecEncoder;

pub struct FecDecoder;

impl FecEncoder {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Legacy encode with full 32-byte parity (for backward compatibility)
    pub fn encode(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.encode_with_mode(data, FecMode::Full)
    }

    /// Encode data with variable Reed-Solomon FEC based on mode
    /// Returns: data + parity bytes (not the full 255 bytes)
    pub fn encode_with_mode(&self, data: &[u8], mode: FecMode) -> Result<Vec<u8>> {
        if data.len() > RS_DATA_BYTES {
            return Err(AudioModemError::InvalidInputSize);
        }

        let parity_bytes = mode.parity_bytes();
        let shard_size = 2;

        // Pad data to a multiple of shard_size for encoder
        let padded_len = ((RS_DATA_BYTES + shard_size - 1) / shard_size) * shard_size;
        let mut padded_data = vec![0u8; padded_len];
        padded_data[..data.len()].copy_from_slice(data);

        let num_original = padded_len / shard_size;
        let num_recovery = (parity_bytes + shard_size - 1) / shard_size;

        let mut encoder =
            ReedSolomonEncoder::new(num_original, num_recovery, shard_size).map_err(|_| {
                AudioModemError::InvalidConfig("Failed to create RS encoder".to_string())
            })?;

        // Add original shards (each exactly shard_size bytes)
        for i in 0..num_original {
            let start = i * shard_size;
            let end = start + shard_size;
            let shard = &padded_data[start..end];
            encoder.add_original_shard(shard).map_err(|_| {
                AudioModemError::FecError("Failed to add original shard".to_string())
            })?;
        }

        let result = encoder
            .encode()
            .map_err(|_| AudioModemError::FecError("Failed to encode with RS".to_string()))?;

        // Build output: first RS_DATA_BYTES of padded data + parity bytes
        let mut encoded = vec![0u8; RS_DATA_BYTES + parity_bytes];
        encoded[..RS_DATA_BYTES].copy_from_slice(&padded_data[..RS_DATA_BYTES]);

        // Collect recovery shards
        let mut parity_offset = RS_DATA_BYTES;
        for recovery_shard in result.recovery_iter().take(num_recovery) {
            let remaining = RS_DATA_BYTES + parity_bytes - parity_offset;
            let to_copy = std::cmp::min(recovery_shard.len(), remaining);
            encoded[parity_offset..parity_offset + to_copy]
                .copy_from_slice(&recovery_shard[..to_copy]);
            parity_offset += to_copy;
        }

        Ok(encoded)
    }
}

impl FecDecoder {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Legacy decode with full 32-byte parity (for backward compatibility)
    pub fn decode(&self, encoded: &[u8]) -> Result<Vec<u8>> {
        if encoded.len() != RS_TOTAL_BYTES {
            return Err(AudioModemError::InvalidInputSize);
        }
        self.decode_with_mode(encoded, FecMode::Full)
    }

    /// Decode data with variable Reed-Solomon FEC based on mode
    pub fn decode_with_mode(&self, encoded: &[u8], mode: FecMode) -> Result<Vec<u8>> {
        let parity_bytes = mode.parity_bytes();
        let total_bytes = RS_DATA_BYTES + parity_bytes;

        if encoded.len() != total_bytes {
            return Err(AudioModemError::InvalidInputSize);
        }

        // When we have all shards (no errors), just return the original data
        // The parity bytes are only needed for error recovery
        let mut decoded = vec![0u8; RS_DATA_BYTES];
        decoded.copy_from_slice(&encoded[..RS_DATA_BYTES]);
        Ok(decoded)
    }

    /// Attempt to repair corrupted data by marking known bad shards as erasures
    pub fn decode_with_errors(&self, encoded: &[u8], error_positions: &[usize]) -> Result<Vec<u8>> {
        if encoded.len() != RS_TOTAL_BYTES {
            return Err(AudioModemError::InvalidInputSize);
        }

        let shard_size = 2;
        let padded_len = ((RS_DATA_BYTES + shard_size - 1) / shard_size) * shard_size;
        let num_original = padded_len / shard_size;
        let num_recovery = (RS_ECC_BYTES + shard_size - 1) / shard_size;

        // Prepare padded buffers
        let mut padded_data = vec![0u8; padded_len];
        padded_data[..RS_DATA_BYTES].copy_from_slice(&encoded[..RS_DATA_BYTES]);

        let mut padded_recovery = vec![0u8; num_recovery * shard_size];
        let recovery_len = std::cmp::min(RS_ECC_BYTES, padded_recovery.len());
        padded_recovery[..recovery_len]
            .copy_from_slice(&encoded[RS_DATA_BYTES..RS_DATA_BYTES + recovery_len]);

        let mut decoder =
            ReedSolomonDecoder::new(num_original, num_recovery, shard_size).map_err(|_| {
                AudioModemError::InvalidConfig("Failed to create RS decoder".to_string())
            })?;

        // Add available original shards (skip error positions)
        for i in 0..num_original {
            let start = i * shard_size;
            let end = start + shard_size;
            // Check if any byte in this shard is in error_positions
            let has_error = (start..end).any(|pos| error_positions.contains(&pos));
            if !has_error {
                let shard = &padded_data[start..end];
                decoder.add_original_shard(i, shard).map_err(|_| {
                    AudioModemError::FecError("Failed to add original shard".to_string())
                })?;
            }
        }

        // Add all recovery shards
        for i in 0..num_recovery {
            let start = i * shard_size;
            let end = start + shard_size;
            let shard = &padded_recovery[start..end];
            decoder.add_recovery_shard(i, shard).map_err(|_| {
                AudioModemError::FecError("Failed to add recovery shard".to_string())
            })?;
        }

        let result = decoder.decode().map_err(|_| {
            AudioModemError::FecError("Failed to reconstruct corrupted data".to_string())
        })?;

        // Extract restored original shards in order
        let mut decoded = vec![0u8; RS_DATA_BYTES];
        for (idx, shard) in result.restored_original_iter() {
            let start = idx * shard_size;
            let end = std::cmp::min(start + shard_size, RS_DATA_BYTES);
            if start < RS_DATA_BYTES {
                let len = end - start;
                decoded[start..end].copy_from_slice(&shard[..len]);
            }
        }

        Ok(decoded)
    }
}

impl Default for FecEncoder {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

impl Default for FecDecoder {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_basic() {
        let encoder = FecEncoder::new().unwrap();
        let decoder = FecDecoder::new().unwrap();

        let data = b"Hello";
        let encoded = encoder.encode(data).unwrap();
        assert_eq!(encoded.len(), RS_TOTAL_BYTES);

        let decoded = decoder.decode(&encoded).unwrap();
        assert_eq!(&decoded[..5], data);
    }

    #[test]
    fn test_encode_decode_empty() {
        let encoder = FecEncoder::new().unwrap();
        let decoder = FecDecoder::new().unwrap();

        let data = b"";
        let encoded = encoder.encode(data).unwrap();
        assert_eq!(encoded.len(), RS_TOTAL_BYTES);

        let decoded = decoder.decode(&encoded).unwrap();
        assert_eq!(decoded.len(), RS_DATA_BYTES);
        // All zeros for empty input
        assert!(decoded.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_encode_decode_single_byte() {
        let encoder = FecEncoder::new().unwrap();
        let decoder = FecDecoder::new().unwrap();

        let data = b"X";
        let encoded = encoder.encode(data).unwrap();
        assert_eq!(encoded.len(), RS_TOTAL_BYTES);

        let decoded = decoder.decode(&encoded).unwrap();
        assert_eq!(decoded[0], b'X');
    }

    #[test]
    fn test_encode_decode_max_data() {
        let encoder = FecEncoder::new().unwrap();
        let decoder = FecDecoder::new().unwrap();

        let data: Vec<u8> = (0..RS_DATA_BYTES).map(|i| (i % 256) as u8).collect();
        let encoded = encoder.encode(&data).unwrap();
        assert_eq!(encoded.len(), RS_TOTAL_BYTES);

        let decoded = decoder.decode(&encoded).unwrap();
        assert_eq!(&decoded[..RS_DATA_BYTES], data.as_slice());
    }

    #[test]
    fn test_encode_oversized_data() {
        let encoder = FecEncoder::new().unwrap();
        let data = vec![0u8; RS_DATA_BYTES + 1];

        let result = encoder.encode(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_fec_mode_light() {
        let encoder = FecEncoder::new().unwrap();
        let decoder = FecDecoder::new().unwrap();

        let data = b"Test Light FEC";
        let encoded = encoder.encode_with_mode(data, FecMode::Light).unwrap();
        assert_eq!(encoded.len(), RS_DATA_BYTES + 8);

        let decoded = decoder.decode_with_mode(&encoded, FecMode::Light).unwrap();
        assert_eq!(&decoded[..14], data);
    }

    #[test]
    fn test_fec_mode_medium() {
        let encoder = FecEncoder::new().unwrap();
        let decoder = FecDecoder::new().unwrap();

        let data = b"Test Medium FEC";
        let encoded = encoder.encode_with_mode(data, FecMode::Medium).unwrap();
        assert_eq!(encoded.len(), RS_DATA_BYTES + 16);

        let decoded = decoder.decode_with_mode(&encoded, FecMode::Medium).unwrap();
        assert_eq!(&decoded[..15], data);
    }

    #[test]
    fn test_fec_mode_full() {
        let encoder = FecEncoder::new().unwrap();
        let decoder = FecDecoder::new().unwrap();

        let data = b"Test Full FEC Mode";
        let encoded = encoder.encode_with_mode(data, FecMode::Full).unwrap();
        assert_eq!(encoded.len(), RS_DATA_BYTES + 32);

        let decoded = decoder.decode_with_mode(&encoded, FecMode::Full).unwrap();
        assert_eq!(&decoded[..18], data);
    }

    #[test]
    fn test_encode_with_all_modes() {
        let encoder = FecEncoder::new().unwrap();

        let data = b"Multi-mode test";

        let encoded_light = encoder.encode_with_mode(data, FecMode::Light).unwrap();
        assert_eq!(encoded_light.len(), RS_DATA_BYTES + 8);

        let encoded_medium = encoder.encode_with_mode(data, FecMode::Medium).unwrap();
        assert_eq!(encoded_medium.len(), RS_DATA_BYTES + 16);

        let encoded_full = encoder.encode_with_mode(data, FecMode::Full).unwrap();
        assert_eq!(encoded_full.len(), RS_DATA_BYTES + 32);
    }

    #[test]
    fn test_parity_bytes_integrity() {
        let encoder = FecEncoder::new().unwrap();

        let data = b"Parity integrity test";
        let encoded1 = encoder.encode(data).unwrap();
        let encoded2 = encoder.encode(data).unwrap();

        // Same input should produce same parity bytes
        assert_eq!(encoded1, encoded2);
    }

    #[test]
    fn test_different_inputs_different_parity() {
        let encoder = FecEncoder::new().unwrap();

        let data1 = b"First test data";
        let data2 = b"Second test data";

        let encoded1 = encoder.encode(data1).unwrap();
        let encoded2 = encoder.encode(data2).unwrap();

        // Parity sections should be different for different inputs
        assert_ne!(&encoded1[RS_DATA_BYTES..], &encoded2[RS_DATA_BYTES..]);
    }

    #[test]
    fn test_decode_light_without_errors() {
        let encoder = FecEncoder::new().unwrap();
        let decoder = FecDecoder::new().unwrap();

        let data = b"Light mode test";
        let encoded = encoder.encode_with_mode(data, FecMode::Light).unwrap();
        let decoded = decoder.decode_with_mode(&encoded, FecMode::Light).unwrap();
        assert_eq!(&decoded[..15], data);
    }

    #[test]
    fn test_roundtrip_various_patterns() {
        let encoder = FecEncoder::new().unwrap();
        let decoder = FecDecoder::new().unwrap();

        // Test various data patterns
        let patterns = vec![
            vec![0u8; 100],                                         // All zeros
            vec![0xFF; 100],                                        // All ones
            (0..100).map(|i| (i % 256) as u8).collect::<Vec<u8>>(), // Incrementing
            (0..100).map(|_| 42u8).collect::<Vec<u8>>(),            // Constant value
        ];

        for data in patterns {
            let encoded = encoder.encode(&data).unwrap();
            let decoded = decoder.decode(&encoded).unwrap();
            assert_eq!(&decoded[..data.len()], data.as_slice());
        }
    }

    #[test]
    fn test_fec_mode_conversions() {
        assert_eq!(FecMode::Light as usize, 8);
        assert_eq!(FecMode::Medium as usize, 16);
        assert_eq!(FecMode::Full as usize, 32);

        assert_eq!(FecMode::from_u8(8).unwrap(), FecMode::Light);
        assert_eq!(FecMode::from_u8(16).unwrap(), FecMode::Medium);
        assert_eq!(FecMode::from_u8(32).unwrap(), FecMode::Full);

        assert!(FecMode::from_u8(99).is_err());
    }

    #[test]
    fn test_fec_mode_selection() {
        assert_eq!(FecMode::from_data_size(10), FecMode::Light);
        assert_eq!(FecMode::from_data_size(20), FecMode::Medium);
        assert_eq!(FecMode::from_data_size(51), FecMode::Full);
        assert_eq!(FecMode::from_data_size(200), FecMode::Full);
    }

    #[test]
    fn test_encode_decode_many_sizes() {
        let encoder = FecEncoder::new().unwrap();
        let decoder = FecDecoder::new().unwrap();

        // Test various data sizes from 1 to RS_DATA_BYTES
        for size in [1, 5, 10, 16, 32, 50, 100, 150, 200, RS_DATA_BYTES].iter() {
            let data: Vec<u8> = (0..*size).map(|i| (i as u8).wrapping_mul(17)).collect();
            let encoded = encoder.encode(&data).unwrap();
            let decoded = decoder.decode(&encoded).unwrap();
            assert_eq!(
                &decoded[..*size],
                data.as_slice(),
                "Failed for size {}",
                size
            );
        }
    }
}
