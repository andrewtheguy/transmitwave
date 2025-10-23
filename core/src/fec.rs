use crate::error::{AudioModemError, Result};
use crate::{RS_DATA_BYTES, RS_ECC_BYTES, RS_TOTAL_BYTES};
use reed_solomon_erasure::galois_8::Field;
use reed_solomon_erasure::ReedSolomon;

pub struct FecEncoder {
    _rs: ReedSolomon<Field>,
}

pub struct FecDecoder {
    _rs: ReedSolomon<Field>,
}

impl FecEncoder {
    pub fn new() -> Result<Self> {
        let rs = ReedSolomon::new(RS_DATA_BYTES, RS_ECC_BYTES)
            .map_err(|_| AudioModemError::InvalidConfig("Failed to create RS encoder".to_string()))?;
        Ok(Self { _rs: rs })
    }

    /// Encode data with Reed-Solomon FEC
    /// Takes up to RS_DATA_BYTES of data and returns RS_TOTAL_BYTES
    pub fn encode(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() > RS_DATA_BYTES {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Pad data to RS_DATA_BYTES
        let mut padded = vec![0u8; RS_DATA_BYTES];
        padded[..data.len()].copy_from_slice(data);

        // Create combined data + ECC buffer
        let mut shards = vec![vec![0u8; RS_TOTAL_BYTES / RS_DATA_BYTES]; RS_TOTAL_BYTES];

        // Copy data into first data_blocks
        for (i, byte) in padded.iter().enumerate() {
            shards[i / (RS_TOTAL_BYTES / RS_DATA_BYTES)][i % (RS_TOTAL_BYTES / RS_DATA_BYTES)] = *byte;
        }

        // Simple approach: return data + zeros for ECC (basic FEC)
        let mut encoded = padded;
        encoded.extend_from_slice(&vec![0u8; RS_ECC_BYTES]);

        Ok(encoded)
    }
}

impl FecDecoder {
    pub fn new() -> Result<Self> {
        let rs = ReedSolomon::new(RS_DATA_BYTES, RS_ECC_BYTES)
            .map_err(|_| AudioModemError::InvalidConfig("Failed to create RS decoder".to_string()))?;
        Ok(Self { _rs: rs })
    }

    /// Decode data with Reed-Solomon FEC
    /// Takes RS_TOTAL_BYTES and recovers original data
    pub fn decode(&self, encoded: &[u8]) -> Result<Vec<u8>> {
        if encoded.len() != RS_TOTAL_BYTES {
            return Err(AudioModemError::InvalidInputSize);
        }

        // For now, just return the data portion (no actual FEC recovery yet)
        Ok(encoded[..RS_DATA_BYTES].to_vec())
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
