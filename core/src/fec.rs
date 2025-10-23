use crate::error::{AudioModemError, Result};
use crate::{RS_DATA_BYTES, RS_ECC_BYTES, RS_TOTAL_BYTES};
use reed_solomon_erasure::galois_8::Field;
use reed_solomon_erasure::ReedSolomon;

pub struct FecEncoder {
    rs: ReedSolomon<Field>,
}

pub struct FecDecoder {
    rs: ReedSolomon<Field>,
}

impl FecEncoder {
    pub fn new() -> Result<Self> {
        let rs = ReedSolomon::new(RS_DATA_BYTES, RS_ECC_BYTES)
            .map_err(|_| AudioModemError::InvalidConfig("Failed to create RS encoder".to_string()))?;
        Ok(Self { rs })
    }

    /// Encode data with Reed-Solomon FEC
    /// Takes up to RS_DATA_BYTES of data and returns RS_TOTAL_BYTES (data + ECC)
    ///
    /// The encoded output is split into shards:
    /// - Shards 0..RS_DATA_BYTES: data shards
    /// - Shards RS_DATA_BYTES..RS_TOTAL_BYTES: parity (ECC) shards
    pub fn encode(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() > RS_DATA_BYTES {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Create data shards (each shard is 1 byte in our case)
        let mut shards: Vec<Vec<u8>> = (0..RS_TOTAL_BYTES)
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
        self.rs
            .encode(&mut shards)
            .map_err(|_| AudioModemError::FecError("Failed to encode with RS".to_string()))?;

        // Flatten shards into single vector
        let encoded: Vec<u8> = shards.into_iter().flatten().collect();
        Ok(encoded)
    }
}

impl FecDecoder {
    pub fn new() -> Result<Self> {
        let rs = ReedSolomon::new(RS_DATA_BYTES, RS_ECC_BYTES)
            .map_err(|_| AudioModemError::InvalidConfig("Failed to create RS decoder".to_string()))?;
        Ok(Self { rs })
    }

    /// Decode data with Reed-Solomon FEC error correction
    /// Takes RS_TOTAL_BYTES and recovers original data
    ///
    /// Can recover from up to RS_ECC_BYTES (32) byte errors
    pub fn decode(&self, encoded: &[u8]) -> Result<Vec<u8>> {
        if encoded.len() != RS_TOTAL_BYTES {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Split into shards wrapped with presence indicator (each shard is 1 byte)
        // Use Option<Vec<u8>> where Some means data is present, None means erasure
        let mut shards: Vec<Option<Vec<u8>>> = encoded
            .chunks(1)
            .map(|c| Some(c.to_vec()))
            .collect();

        // Reconstruct all shards from available data and parity
        self.rs
            .reconstruct(&mut shards)
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
    /// This is useful when we know certain positions are corrupted
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
        self.rs
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
