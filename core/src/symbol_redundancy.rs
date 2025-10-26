use crate::error::{AudioModemError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolRedundancyMode {
    None,
    Parity,
}

const GF16_EXP: [u8; 30] = [
    1, 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1, 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15,
    13, 9,
];

const GF16_LOG: [i8; 16] = [-1, 0, 1, 4, 2, 8, 5, 10, 3, 14, 9, 7, 6, 13, 11, 12];
const RS_GEN_G1: u8 = 6; // alpha^1 + alpha^2
const RS_GEN_G0: u8 = 8; // alpha^1 * alpha^2

pub fn encode_symbol_bytes(data: &[u8], mode: SymbolRedundancyMode) -> Vec<u8> {
    match mode {
        SymbolRedundancyMode::None => data.to_vec(),
        SymbolRedundancyMode::Parity => encode_with_rs16(data),
    }
}

pub fn decode_symbol_bytes(bytes: &[u8], mode: SymbolRedundancyMode) -> Result<Vec<u8>> {
    match mode {
        SymbolRedundancyMode::None => Ok(bytes.to_vec()),
        SymbolRedundancyMode::Parity => decode_with_rs16(bytes),
    }
}

fn encode_with_rs16(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(((data.len() + 1) / 2) * 3);
    for chunk in data.chunks(2) {
        let d0 = chunk.get(0).copied().unwrap_or(0);
        let d1 = chunk.get(1).copied().unwrap_or(0);
        let data_nibbles = [(d0 >> 4) & 0x0F, d0 & 0x0F, (d1 >> 4) & 0x0F, d1 & 0x0F];
        let parity = rs_encode_4_data_symbols(&data_nibbles);
        output.push(((data_nibbles[0] << 4) | data_nibbles[1]) as u8);
        output.push(((data_nibbles[2] << 4) | data_nibbles[3]) as u8);
        output.push(((parity[0] << 4) | parity[1]) as u8);
    }
    output
}

fn decode_with_rs16(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() % 3 != 0 {
        return Err(AudioModemError::InvalidInputSize);
    }

    let mut output = Vec::with_capacity(bytes.len() / 3 * 2);
    for chunk in bytes.chunks(3) {
        let mut nibbles = [
            (chunk[0] >> 4) & 0x0F,
            chunk[0] & 0x0F,
            (chunk[1] >> 4) & 0x0F,
            chunk[1] & 0x0F,
            (chunk[2] >> 4) & 0x0F,
            chunk[2] & 0x0F,
        ];
        rs_correct_single_symbol(&mut nibbles);
        output.push((nibbles[0] << 4) | nibbles[1]);
        output.push((nibbles[2] << 4) | nibbles[3]);
    }

    Ok(output)
}

fn rs_encode_4_data_symbols(data: &[u8; 4]) -> [u8; 2] {
    let mut parity = [0u8; 2];
    for &symbol in data {
        let feedback = symbol ^ parity[0];
        parity[0] = parity[1] ^ gf16_mul(feedback, RS_GEN_G1);
        parity[1] = gf16_mul(feedback, RS_GEN_G0);
    }
    parity
}

fn rs_correct_single_symbol(nibbles: &mut [u8; 6]) {
    let mut data = [nibbles[0], nibbles[1], nibbles[2], nibbles[3]];
    let parity = [nibbles[4], nibbles[5]];
    let expected = rs_encode_4_data_symbols(&data);

    if expected == parity {
        return;
    }

    for idx in 0..4 {
        let original = data[idx];
        for cand in 0..16 {
            if cand == original {
                continue;
            }
            data[idx] = cand;
            if rs_encode_4_data_symbols(&data) == parity {
                nibbles[0] = data[0];
                nibbles[1] = data[1];
                nibbles[2] = data[2];
                nibbles[3] = data[3];
                nibbles[4] = parity[0];
                nibbles[5] = parity[1];
                return;
            }
        }
        data[idx] = original;
    }

    nibbles[0] = data[0];
    nibbles[1] = data[1];
    nibbles[2] = data[2];
    nibbles[3] = data[3];
    nibbles[4] = expected[0];
    nibbles[5] = expected[1];
}

fn gf16_mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let log_a = GF16_LOG[a as usize] as i32;
    let log_b = GF16_LOG[b as usize] as i32;
    let idx = ((log_a + log_b) % 15) as usize;
    GF16_EXP[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parity_encode_decode_roundtrip() {
        let data = b"Parity layer test data";
        let encoded = encode_symbol_bytes(data, SymbolRedundancyMode::Parity);
        let decoded = decode_symbol_bytes(&encoded, SymbolRedundancyMode::Parity).unwrap();
        assert_eq!(&decoded[..data.len()], data);
    }

    #[test]
    fn test_single_nibble_error_correction() {
        let data = [0xAB, 0xCD, 0xEF, 0x01];
        let encoded = encode_symbol_bytes(&data, SymbolRedundancyMode::Parity);
        let mut corrupted = encoded.clone();
        corrupted[1] ^= 0x10; // flip high nibble of byte 1
        let decoded = decode_symbol_bytes(&corrupted, SymbolRedundancyMode::Parity).unwrap();
        assert_eq!(&decoded[..data.len()], &data);
    }

    #[test]
    fn test_no_parity_mode_passthrough() {
        let bytes = vec![1, 2, 3, 4, 5, 6];
        assert_eq!(
            encode_symbol_bytes(&bytes, SymbolRedundancyMode::None),
            bytes
        );
        assert_eq!(
            decode_symbol_bytes(&bytes, SymbolRedundancyMode::None).unwrap(),
            bytes
        );
    }
}
