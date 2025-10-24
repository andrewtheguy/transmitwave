use crate::error::{AudioModemError, Result};
use crate::{RS_DATA_BYTES, RS_ECC_BYTES, RS_TOTAL_BYTES};
use reed_solomon_erasure::galois_8::Field;
use reed_solomon_erasure::ReedSolomon;

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
            _ => Err(AudioModemError::InvalidConfig("Invalid FEC mode".to_string())),
        }
    }

    /// Convert to byte value
    pub fn to_u8(&self) -> u8 {
        *self as u8
    }
}

pub struct FecEncoder {
    rs_light: ReedSolomon<Field>,
    rs_medium: ReedSolomon<Field>,
    rs_full: ReedSolomon<Field>,
}

pub struct FecDecoder {
    rs_light: ReedSolomon<Field>,
    rs_medium: ReedSolomon<Field>,
    rs_full: ReedSolomon<Field>,
}

impl FecEncoder {
    pub fn new() -> Result<Self> {
        let rs_light = ReedSolomon::new(RS_DATA_BYTES, 8)
            .map_err(|_| AudioModemError::InvalidConfig("Failed to create RS light encoder".to_string()))?;
        let rs_medium = ReedSolomon::new(RS_DATA_BYTES, 16)
            .map_err(|_| AudioModemError::InvalidConfig("Failed to create RS medium encoder".to_string()))?;
        let rs_full = ReedSolomon::new(RS_DATA_BYTES, RS_ECC_BYTES)
            .map_err(|_| AudioModemError::InvalidConfig("Failed to create RS full encoder".to_string()))?;
        Ok(Self { rs_light, rs_medium, rs_full })
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
        let total_bytes = RS_DATA_BYTES + parity_bytes;
        let rs = match mode {
            FecMode::Light => &self.rs_light,
            FecMode::Medium => &self.rs_medium,
            FecMode::Full => &self.rs_full,
        };

        // Create data shards (each shard is 1 byte in our case)
        let mut shards: Vec<Vec<u8>> = (0..total_bytes)
            .map(|i| {
                if i < RS_DATA_BYTES {
                    // Data shards: fill with data, pad with zeros
                    if i < data.len() {
                        vec![data[i]]
                    } else {
                        vec![0u8]
                    }
                } else {
                    // Parity shards: initially empty (will be computed)
                    vec![0u8]
                }
            })
            .collect();

        // Encode: compute parity shards
        rs.encode(&mut shards)
            .map_err(|_| AudioModemError::FecError("Failed to encode with RS".to_string()))?;

        // Flatten shards into single vector
        let encoded: Vec<u8> = shards.into_iter().flatten().collect();
        Ok(encoded)
    }
}

impl FecDecoder {
    pub fn new() -> Result<Self> {
        let rs_light = ReedSolomon::new(RS_DATA_BYTES, 8)
            .map_err(|_| AudioModemError::InvalidConfig("Failed to create RS light decoder".to_string()))?;
        let rs_medium = ReedSolomon::new(RS_DATA_BYTES, 16)
            .map_err(|_| AudioModemError::InvalidConfig("Failed to create RS medium decoder".to_string()))?;
        let rs_full = ReedSolomon::new(RS_DATA_BYTES, RS_ECC_BYTES)
            .map_err(|_| AudioModemError::InvalidConfig("Failed to create RS full decoder".to_string()))?;
        Ok(Self { rs_light, rs_medium, rs_full })
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

        let rs = match mode {
            FecMode::Light => &self.rs_light,
            FecMode::Medium => &self.rs_medium,
            FecMode::Full => &self.rs_full,
        };

        // Split into shards wrapped with presence indicator (each shard is 1 byte)
        let mut shards: Vec<Option<Vec<u8>>> = encoded
            .chunks(1)
            .map(|c| Some(c.to_vec()))
            .collect();

        // Reconstruct all shards from available data and parity
        rs.reconstruct(&mut shards)
            .map_err(|_| AudioModemError::FecError("Failed to decode with RS".to_string()))?;

        // Extract data shards (first RS_DATA_BYTES shards)
        let decoded: Vec<u8> = shards[0..RS_DATA_BYTES]
            .iter()
            .filter_map(|shard| shard.as_ref())
            .flat_map(|shard| shard.clone())
            .collect();

        Ok(decoded)
    }

    /// Attempt to repair corrupted data by marking known bad shards as erasures
    pub fn decode_with_errors(&self, encoded: &[u8], error_positions: &[usize]) -> Result<Vec<u8>> {
        if encoded.len() != RS_TOTAL_BYTES {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Split into shards with error marking
        let mut shards: Vec<Option<Vec<u8>>> = encoded
            .chunks(1)
            .enumerate()
            .map(|(idx, c)| {
                if error_positions.contains(&idx) {
                    None  // Mark as erasure
                } else {
                    Some(c.to_vec())
                }
            })
            .collect();

        // Reconstruct
        self.rs_full
            .reconstruct(&mut shards)
            .map_err(|_| AudioModemError::FecError("Failed to reconstruct corrupted data".to_string()))?;

        // Extract data shards
        let decoded: Vec<u8> = shards[0..RS_DATA_BYTES]
            .iter()
            .filter_map(|shard| shard.as_ref())
            .flat_map(|shard| shard.clone())
            .collect();

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
    fn test_encode_decode() {
        let encoder = FecEncoder::new().unwrap();
        let decoder = FecDecoder::new().unwrap();

        let data = b"Hello";
        let encoded = encoder.encode(data).unwrap();
        assert_eq!(encoded.len(), RS_TOTAL_BYTES);

        let decoded = decoder.decode(&encoded).unwrap();
        assert_eq!(&decoded[..5], data);
    }
}
