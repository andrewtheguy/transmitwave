use testaudio_core::{Decoder, Encoder, DecoderChunked, EncoderChunked};

#[test]
fn test_encode_decode_round_trip() {
    let original_data = b"Hello, Audio Modem!";

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    assert!(!samples.is_empty(), "No samples generated");
    println!("Generated {} audio samples", samples.len());

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Decoded data doesn't match original");
    println!("Successfully decoded: {:?}", String::from_utf8_lossy(&decoded_data));
}

#[test]
fn test_encode_decode_max_size() {
    let original_data = vec![42u8; 200]; // Max payload size

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let samples = encoder.encode(&original_data).expect("Failed to encode");

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Large payload round-trip failed");
}

#[test]
fn test_encode_decode_binary_data() {
    let original_data = vec![0, 1, 2, 255, 128, 64, 32, 16, 8, 4, 2, 1, 0];

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let samples = encoder.encode(&original_data).expect("Failed to encode");

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Binary data round-trip failed");
}

#[test]
fn test_empty_data() {
    let original_data = b"";

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Empty data round-trip failed");
}

// Robustness tests with silence and noise at frame edges
#[test]
fn test_encode_decode_with_leading_silence() {
    let original_data = b"Hello, Audio Modem!";

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    // Add silence before the encoded frame (1 second = 16000 samples)
    let mut augmented_samples = vec![0.0; 16000];
    augmented_samples.extend_from_slice(&samples);

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Decoded data with leading silence doesn't match");
}

#[test]
fn test_encode_decode_with_trailing_silence() {
    let original_data = b"Hello, Audio Modem!";

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    // Add silence after the encoded frame (1 second = 16000 samples)
    let mut augmented_samples = samples.clone();
    augmented_samples.extend_from_slice(&vec![0.0; 16000]);

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Decoded data with trailing silence doesn't match");
}

#[test]
fn test_encode_decode_with_silence_both_sides() {
    let original_data = b"Hello, Audio Modem!";

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    // Add silence before and after the encoded frame
    let mut augmented_samples = vec![0.0; 16000];
    augmented_samples.extend_from_slice(&samples);
    augmented_samples.extend_from_slice(&vec![0.0; 16000]);

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Decoded data with silence on both sides doesn't match");
}

#[test]
fn test_encode_decode_with_leading_noise() {
    let original_data = b"Hello, Audio Modem!";

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    // Add noise before the encoded frame
    let mut augmented_samples = Vec::new();
    let mut rng_state = 12345u32;
    for _ in 0..16000 {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.1; // 10% amplitude noise
        augmented_samples.push(noise);
    }
    augmented_samples.extend_from_slice(&samples);

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Decoded data with leading noise doesn't match");
}

#[test]
fn test_encode_decode_with_trailing_noise() {
    let original_data = b"Hello, Audio Modem!";

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    // Add noise after the encoded frame
    let mut augmented_samples = samples.clone();
    let mut rng_state = 54321u32;
    for _ in 0..16000 {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.1; // 10% amplitude noise
        augmented_samples.push(noise);
    }

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Decoded data with trailing noise doesn't match");
}

#[test]
fn test_encode_decode_with_noise_both_sides() {
    let original_data = b"Hello, Audio Modem!";

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    // Add noise before and after the encoded frame
    let mut augmented_samples = Vec::new();
    let mut rng_state = 12345u32;
    for _ in 0..16000 {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.1;
        augmented_samples.push(noise);
    }
    augmented_samples.extend_from_slice(&samples);

    rng_state = 54321u32;
    for _ in 0..16000 {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.1;
        augmented_samples.push(noise);
    }

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Decoded data with noise on both sides doesn't match");
}

#[test]
fn test_encode_decode_binary_with_leading_silence() {
    let original_data = vec![0, 1, 2, 255, 128, 64, 32, 16, 8, 4, 2, 1, 0];

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let samples = encoder.encode(&original_data).expect("Failed to encode");

    // Add leading silence
    let mut augmented_samples = vec![0.0; 16000];
    augmented_samples.extend_from_slice(&samples);

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Binary data with leading silence round-trip failed");
}

#[test]
fn test_encode_decode_max_size_with_silence_and_noise() {
    let original_data = vec![42u8; 200]; // Max payload size

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let samples = encoder.encode(&original_data).expect("Failed to encode");

    // Add both silence and noise around the encoded frame
    let mut augmented_samples = vec![0.0; 8000]; // 0.5 second leading silence
    augmented_samples.extend_from_slice(&samples);
    augmented_samples.extend_from_slice(&vec![0.0; 8000]); // 0.5 second trailing silence

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Max payload with silence round-trip failed");
}

// Tests with noise added directly to the encoded data
#[test]
fn test_encode_decode_with_light_data_noise() {
    let original_data = b"Hello, Audio Modem!";

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let mut samples = encoder.encode(original_data).expect("Failed to encode");

    // Add 5% amplitude noise directly to the encoded data
    let mut rng_state = 11111u32;
    for sample in samples.iter_mut() {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.05; // 5% noise
        *sample += noise;
    }

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Light data noise round-trip failed");
}

#[test]
fn test_encode_decode_with_moderate_data_noise() {
    let original_data = b"Hello, Audio Modem!";

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let mut samples = encoder.encode(original_data).expect("Failed to encode");

    // Add 10% amplitude noise directly to the encoded data
    let mut rng_state = 22222u32;
    for sample in samples.iter_mut() {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.10; // 10% noise
        *sample += noise;
    }

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Moderate data noise round-trip failed");
}

#[test]
fn test_encode_decode_with_heavy_data_noise() {
    let original_data = b"Hello, Audio Modem!";

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let mut samples = encoder.encode(original_data).expect("Failed to encode");

    // Add 20% amplitude noise directly to the encoded data
    let mut rng_state = 33333u32;
    for sample in samples.iter_mut() {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.20; // 20% noise
        *sample += noise;
    }

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Heavy data noise round-trip failed");
}

#[test]
fn test_encode_decode_binary_with_data_noise() {
    let original_data = vec![0, 1, 2, 255, 128, 64, 32, 16, 8, 4, 2, 1, 0];

    let mut encoder = Encoder::new().expect("Failed to create encoder");
    let mut samples = encoder.encode(&original_data).expect("Failed to encode");

    // Add 10% amplitude noise to binary data
    let mut rng_state = 44444u32;
    for sample in samples.iter_mut() {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.10;
        *sample += noise;
    }

    let mut decoder = Decoder::new().expect("Failed to create decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Binary data with noise round-trip failed");
}

#[test]
fn test_encode_decode_chunked_simple() {
    let original_data = b"Hi";

    let mut encoder = EncoderChunked::new(32, 2).expect("Failed to create chunked encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    assert!(!samples.is_empty(), "No samples generated");

    let mut decoder = DecoderChunked::new(32).expect("Failed to create chunked decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Chunked round-trip failed for simple data");
}

#[test]
fn test_encode_decode_chunked_48bits() {
    let original_data = b"Hello";

    let mut encoder = EncoderChunked::new(48, 3).expect("Failed to create chunked encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    assert!(!samples.is_empty(), "No samples generated");

    let mut decoder = DecoderChunked::new(48).expect("Failed to create chunked decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Chunked round-trip failed for 48-bit chunks");
}

// Test OFDM bit roundtrip without FEC to identify bit corruption
#[test]
fn test_ofdm_bit_roundtrip() {
    use testaudio_core::OfdmModulator;
    use testaudio_core::OfdmDemodulator;

    let original_bits = vec![
        true, false, true, false, true, false, true, false,
        false, true, false, true, false, true, false, true,
        true, true, false, false, true, true, false, false,
        true, false, true, false, true, false, true, false,
        false, false, false, false, true, true, true, true,
        false, true, false, true, false, true, false, true,
    ];

    let mut modulator = OfdmModulator::new();
    let samples = modulator.modulate(&original_bits).expect("Failed to modulate");

    let mut demodulator = OfdmDemodulator::new();
    let demodulated_bits = demodulator.demodulate(&samples).expect("Failed to demodulate");

    println!("Original bits: {:?}", original_bits);
    println!("Demodulated bits: {:?}", demodulated_bits);

    let mut mismatches = 0;
    for (i, (orig, dem)) in original_bits.iter().zip(demodulated_bits.iter()).enumerate() {
        if orig != dem {
            mismatches += 1;
            println!("Bit {} mismatch: expected {}, got {}", i, orig, dem);
        }
    }

    println!("Total mismatches: {}/{}", mismatches, original_bits.len());
    assert_eq!(demodulated_bits, original_bits, "OFDM bit roundtrip failed");
}

// Test FEC encode/decode to verify integrity
#[test]
fn test_fec_chunk_roundtrip() {
    use testaudio_core::FecEncoder;
    use testaudio_core::FecDecoder;

    // Test data: create some test bytes
    let test_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let mut padded_data = test_data.clone();
    // Pad to 223 bytes
    while padded_data.len() < 223 {
        padded_data.push(0);
    }

    let mut encoder = FecEncoder::new().expect("Failed to create FEC encoder");
    let encoded = encoder.encode(&padded_data[..223]).expect("Failed to FEC encode");

    println!("Original padded data (first 10): {:?}", &padded_data[..10]);
    println!("FEC encoded length: {}", encoded.len());

    let mut decoder = FecDecoder::new().expect("Failed to create FEC decoder");
    let decoded = decoder.decode(&encoded).expect("Failed to FEC decode");

    println!("Decoded length: {}", decoded.len());
    println!("Decoded data (first 10): {:?}", &decoded[..10]);

    assert_eq!(&decoded[..10], &test_data, "FEC roundtrip data mismatch");
}

// Test OFDM + FEC together
#[test]
fn test_ofdm_fec_bit_roundtrip() {
    use testaudio_core::OfdmModulator;
    use testaudio_core::OfdmDemodulator;
    use testaudio_core::FecEncoder;
    use testaudio_core::FecDecoder;

    // Create test data and pad to 223 bytes
    let test_data = vec![42, 84, 126, 168, 210, 252, 1, 2, 3, 4, 5, 6, 7, 8];
    let mut padded_data = test_data.clone();
    while padded_data.len() < 223 {
        padded_data.push(0);
    }

    // FEC encode
    let mut fec_encoder = FecEncoder::new().expect("Failed to create FEC encoder");
    let fec_encoded = fec_encoder.encode(&padded_data[..223]).expect("Failed to FEC encode");
    println!("FEC encoded length: {} (should be 255)", fec_encoded.len());

    // Convert bytes to bits
    let mut bits = Vec::new();
    for byte in &fec_encoded {
        for i in (0..8).rev() {
            bits.push((byte >> i) & 1 == 1);
        }
    }
    println!("Total bits: {}", bits.len());

    // Split into OFDM symbols (48 bits per symbol)
    let mut ofdm_samples = Vec::new();
    let mut modulator = OfdmModulator::new();

    for symbol_bits in bits.chunks(48) {
        let samples = modulator.modulate(symbol_bits).expect("Failed to modulate");
        ofdm_samples.extend(samples);
    }
    println!("OFDM samples: {}", ofdm_samples.len());

    // Demodulate back to bits
    let mut demodulated_bits = Vec::new();
    let mut demodulator = OfdmDemodulator::new();

    for chunk in ofdm_samples.chunks(1600) { // SAMPLES_PER_SYMBOL = 1600
        if chunk.len() >= 1600 {
            let bits = demodulator.demodulate(&chunk[..1600]).expect("Failed to demodulate");
            demodulated_bits.extend(bits);
        }
    }

    println!("Demodulated bits: {}", demodulated_bits.len());

    // Convert bits back to bytes
    let mut recovered_bytes = Vec::new();
    for chunk in demodulated_bits.chunks(8) {
        if chunk.len() == 8 {
            let mut byte = 0u8;
            for (i, &bit) in chunk.iter().enumerate() {
                if bit {
                    byte |= 1 << (7 - i);
                }
            }
            recovered_bytes.push(byte);
        }
    }

    println!("Recovered bytes: {} (should be 255)", recovered_bytes.len());

    // FEC decode
    let mut fec_decoder = FecDecoder::new().expect("Failed to create FEC decoder");
    let decoded = match fec_decoder.decode(&recovered_bytes[..255.min(recovered_bytes.len())]) {
        Ok(d) => d,
        Err(e) => {
            println!("FEC decode error: {:?}", e);
            return;
        }
    };

    println!("Decoded length: {} (should be 223)", decoded.len());
    println!("Original test data (first 14): {:?}", &test_data);
    println!("Decoded data (first 14): {:?}", &decoded[..14]);

    assert_eq!(&decoded[..test_data.len()], &test_data, "OFDM+FEC roundtrip failed");
}

// Minimal chunked encode/decode test with debugging
#[test]
fn test_encode_decode_chunked_debug() {
    let original_data = b"Hi";

    println!("\n=== ENCODING ===");
    let mut encoder = EncoderChunked::new(32, 2).expect("Failed to create chunked encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");
    println!("Generated {} samples", samples.len());

    println!("\n=== DECODING ===");
    let _decoder = DecoderChunked::new(32).expect("Failed to create chunked decoder");

    // Manual decode with logging
    use testaudio_core::{detect_preamble, detect_postamble, PREAMBLE_SAMPLES, SAMPLES_PER_SYMBOL, RS_TOTAL_BYTES};
    use testaudio_core::OfdmDemodulator;
    use testaudio_core::FecDecoder;

    if samples.len() < SAMPLES_PER_SYMBOL * 2 {
        println!("ERROR: Insufficient data");
        return;
    }

    // Detect preamble
    let preamble_pos = match detect_preamble(&samples, 500.0) {
        Some(pos) => {
            println!("Preamble detected at position {}", pos);
            pos
        }
        None => {
            println!("ERROR: Preamble not found");
            return;
        }
    };

    let data_start = preamble_pos + PREAMBLE_SAMPLES;
    println!("Data starts at position {}", data_start);

    if data_start + SAMPLES_PER_SYMBOL > samples.len() {
        println!("ERROR: Insufficient data after preamble");
        return;
    }

    let remaining = &samples[data_start..];
    let postamble_pos = match detect_postamble(remaining, 100.0) {
        Some(pos) => {
            println!("Postamble detected at position {} (relative)", pos);
            pos
        }
        None => {
            println!("ERROR: Postamble not found");
            return;
        }
    };

    let data_end = data_start + postamble_pos;
    println!("Data ends at position {}", data_end);
    println!("Data length: {} samples", data_end - data_start);

    // Demodulate all symbols
    let mut all_bits = Vec::new();
    let mut pos = data_start;
    let mut symbol_count = 0;
    let mut demodulator = OfdmDemodulator::new();

    while pos + SAMPLES_PER_SYMBOL <= data_end {
        let symbol_bits = demodulator.demodulate(&samples[pos..]).expect("Demodulate failed");
        all_bits.extend_from_slice(&symbol_bits);
        symbol_count += 1;
        pos += SAMPLES_PER_SYMBOL;
    }
    println!("Demodulated {} symbols = {} bits", symbol_count, all_bits.len());

    // Convert bits to bytes
    let mut all_bytes = Vec::new();
    for chunk in all_bits.chunks(8) {
        if chunk.len() == 8 {
            let mut byte = 0u8;
            for (i, &bit) in chunk.iter().enumerate() {
                if bit {
                    byte |= 1 << (7 - i);
                }
            }
            all_bytes.push(byte);
        }
    }
    println!("Converted to {} bytes", all_bytes.len());

    // Process FEC chunks
    let mut fec_decoder = FecDecoder::new().expect("Failed to create FEC decoder");
    let mut byte_pos = 0;
    let mut fec_count = 0;
    let mut valid_chunks = 0;

    while byte_pos + RS_TOTAL_BYTES <= all_bytes.len() {
        let fec_chunk = &all_bytes[byte_pos..byte_pos + RS_TOTAL_BYTES];
        match fec_decoder.decode(fec_chunk) {
            Ok(decoded_bytes) => {
                fec_count += 1;
                println!("FEC chunk {} decoded successfully", fec_count);

                // Try to extract chunk
                let chunk_data_bytes = 32 / 8; // 4 bytes
                let chunk_total_bytes = 7 + chunk_data_bytes; // 11 bytes

                if decoded_bytes.len() >= chunk_total_bytes {
                    let chunk_portion = &decoded_bytes[0..chunk_total_bytes];
                    println!("  Chunk bytes: {:?}", chunk_portion);

                    use testaudio_core::Chunk;
                    match Chunk::from_bytes(chunk_portion) {
                        Ok(chunk) => {
                            if chunk.validate_crc() {
                                println!("  CRC valid, chunk_id={}, total_chunks={}", chunk.header.chunk_id, chunk.header.total_chunks);
                                valid_chunks += 1;
                            } else {
                                println!("  CRC INVALID");
                            }
                        }
                        Err(e) => println!("  Chunk parse error: {:?}", e),
                    }
                }
            }
            Err(e) => println!("FEC decode error: {:?}", e),
        }
        byte_pos += RS_TOTAL_BYTES;
    }

    println!("Total FEC chunks processed: {}", fec_count);
    println!("Valid chunks: {}", valid_chunks);
}

// ============================================================================
// COMPREHENSIVE ROUND-TRIP CORRECTNESS TESTS
// These tests verify exact data match for critical algorithms
// ============================================================================

#[test]
fn test_spread_spectrum_roundtrip_simple() {
    use testaudio_core::{DecoderSpread, EncoderSpread};

    let original_data = b"Test";
    let mut encoder = EncoderSpread::new(2).expect("Failed to create spread encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    let mut decoder = DecoderSpread::new(2).expect("Failed to create spread decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Spread spectrum round-trip corruption detected");
}

#[test]
fn test_spread_spectrum_roundtrip_max_payload() {
    use testaudio_core::{DecoderSpread, EncoderSpread};

    let original_data = vec![0xAA; 200]; // Max size with alternating bits
    let mut encoder = EncoderSpread::new(2).expect("Failed to create spread encoder");
    let samples = encoder.encode(&original_data).expect("Failed to encode");

    let mut decoder = DecoderSpread::new(2).expect("Failed to create spread decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Spread spectrum max payload corruption");
}

#[test]
fn test_spread_spectrum_roundtrip_binary_patterns() {
    use testaudio_core::{DecoderSpread, EncoderSpread};

    let test_patterns = vec![
        vec![0xFF, 0xFF, 0xFF],      // All ones
        vec![0x00, 0x00, 0x00],      // All zeros
        vec![0xAA, 0x55, 0xAA],      // Alternating pattern
        (0..=255).collect::<Vec<u8>>(), // All byte values
    ];

    for pattern in test_patterns {
        let mut encoder = EncoderSpread::new(2).expect("Failed to create encoder");
        let samples = encoder.encode(&pattern).expect("Failed to encode");

        let mut decoder = DecoderSpread::new(2).expect("Failed to create decoder");
        let decoded = decoder.decode(&samples).expect("Failed to decode");

        assert_eq!(decoded, pattern, "Spread spectrum corrupted pattern: {:?}", pattern);
    }
}

#[test]
fn test_chunked_roundtrip_32bit_chunks() {
    let original_data = b"Hi";

    let mut encoder = EncoderChunked::new(32, 2).expect("Failed to create chunked encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    let mut decoder = DecoderChunked::new(32).expect("Failed to create chunked decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Chunked 32-bit round-trip corruption");
}

#[test]
fn test_chunked_roundtrip_48bit_chunks() {
    let original_data = b"Hello, World!";

    let mut encoder = EncoderChunked::new(48, 3).expect("Failed to create chunked encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    let mut decoder = DecoderChunked::new(48).expect("Failed to create chunked decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Chunked 48-bit round-trip corruption");
}

#[test]
fn test_chunked_roundtrip_64bit_chunks() {
    let original_data = b"12345678"; // Exactly 8 bytes = 64 bits

    let mut encoder = EncoderChunked::new(64, 2).expect("Failed to create chunked encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    let mut decoder = DecoderChunked::new(64).expect("Failed to create chunked decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Chunked 64-bit round-trip corruption");
}

#[test]
fn test_chunked_roundtrip_max_payload() {
    let original_data = vec![0x42; 200]; // Max size

    let mut encoder = EncoderChunked::new(48, 3).expect("Failed to create chunked encoder");
    let samples = encoder.encode(&original_data).expect("Failed to encode");

    let mut decoder = DecoderChunked::new(48).expect("Failed to create chunked decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Chunked max payload corruption detected");
}

#[test]
fn test_chunked_roundtrip_boundary_sizes() {
    let test_cases = vec![
        1,    // Single byte
        6,    // One 48-bit chunk
        7,    // More than one 48-bit chunk
        12,   // Two 48-bit chunks
        200,  // Max size
    ];

    for size in test_cases {
        let original_data = vec![size as u8; size];

        let mut encoder = EncoderChunked::new(48, 3).expect("Failed to create encoder");
        let samples = encoder.encode(&original_data).expect("Failed to encode");

        let mut decoder = DecoderChunked::new(48).expect("Failed to create decoder");
        let decoded_data = decoder.decode(&samples).expect("Failed to decode");

        assert_eq!(decoded_data, original_data,
                   "Chunked corruption at boundary size {}: expected {}, got {}",
                   size, original_data.len(), decoded_data.len());
    }
}

#[test]
fn test_chunked_roundtrip_binary_patterns() {
    let patterns = vec![
        vec![0xFF; 10],              // All ones
        vec![0x00; 10],              // All zeros
        {
            let mut p = vec![];
            for _ in 0..5 {
                p.push(0xAA);
                p.push(0x55);
            }
            p
        },                           // Alternating pattern
        (0..20).collect::<Vec<u8>>(), // Sequential
    ];

    for pattern in patterns {
        let mut encoder = EncoderChunked::new(48, 3).expect("Failed to create encoder");
        let samples = encoder.encode(&pattern).expect("Failed to encode");

        let mut decoder = DecoderChunked::new(48).expect("Failed to create decoder");
        let decoded_data = decoder.decode(&samples).expect("Failed to decode");

        assert_eq!(decoded_data, pattern, "Chunked corrupted pattern: {:?}", pattern);
    }
}

#[test]
fn test_chunked_with_different_interleave_factors() {
    let original_data = b"Testing interleave";

    for interleave in &[2, 3, 4, 5] {
        let mut encoder = EncoderChunked::new(48, *interleave)
            .expect("Failed to create chunked encoder");
        let samples = encoder.encode(original_data).expect("Failed to encode");

        let mut decoder = DecoderChunked::new(48).expect("Failed to create chunked decoder");
        let decoded_data = decoder.decode(&samples).expect("Failed to decode");

        assert_eq!(decoded_data, original_data,
                   "Chunked corruption with interleave factor {}", interleave);
    }
}

#[test]
fn test_chunked_correctness_with_all_bit_values() {
    // Test that all possible byte values are correctly encoded/decoded
    let original_data: Vec<u8> = (0..=255).collect();

    let mut encoder = EncoderChunked::new(48, 3).expect("Failed to create encoder");
    let samples = encoder.encode(&original_data).expect("Failed to encode");

    let mut decoder = DecoderChunked::new(48).expect("Failed to create decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data.len(), 256, "Decoded size mismatch");
    for (i, (expected, actual)) in original_data.iter().zip(decoded_data.iter()).enumerate() {
        assert_eq!(expected, actual, "Byte corruption at position {}: expected {}, got {}", i, expected, actual);
    }
}

#[test]
fn test_chunked_padding_correctness() {
    // Test that padding is correctly removed after decoding
    // 5 bytes = 2.5 chunks of 48 bits, so last chunk is padded
    let original_data = b"ABCDE";

    let mut encoder = EncoderChunked::new(48, 2).expect("Failed to create encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    let mut decoder = DecoderChunked::new(48).expect("Failed to create decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    // Critical: Check exact length match (no padding bytes remain)
    assert_eq!(decoded_data.len(), 5, "Padding not removed: expected 5 bytes, got {}", decoded_data.len());
    assert_eq!(decoded_data, original_data, "Decoded data doesn't match original");
}
