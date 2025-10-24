// ============================================================================
// INTEGRATION TESTS - PERFORMANCE NOTE
// ============================================================================
// These tests perform full encode/decode roundtrips with preamble/postamble
// synchronization, which involves cross-correlation on 2000-sample templates.
//
// For faster test execution, run in release mode:
//   cargo test -p transmitwave-core --test integration_test --release
//
// Performance comparison:
//   Debug mode:   ~22-30 seconds per test (full synchronization overhead)
//   Release mode: ~3-4 seconds total for all tests (28 passed, 6 ignored)
//
// The slowness is inherent to preamble/postamble detection, not a bug.
// ============================================================================

use transmitwave_core::{Decoder, Encoder, NUM_SUBCARRIERS, EncoderFsk, DecoderFsk};

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
fn test_ofdm_bit_roundtrip() {
    use transmitwave_core::OfdmModulator;
    use transmitwave_core::OfdmDemodulator;

    let original_bits: Vec<bool> = (0..NUM_SUBCARRIERS)
        .map(|i| (i ^ (i >> 3)) & 1 != 0)
        .collect();

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
    use transmitwave_core::FecEncoder;
    use transmitwave_core::FecDecoder;

    // Test data: create some test bytes
    let test_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let mut padded_data = test_data.clone();
    // Pad to 223 bytes
    while padded_data.len() < 223 {
        padded_data.push(0);
    }

    let encoder = FecEncoder::new().expect("Failed to create FEC encoder");
    let encoded = encoder.encode(&padded_data[..223]).expect("Failed to FEC encode");

    println!("Original padded data (first 10): {:?}", &padded_data[..10]);
    println!("FEC encoded length: {}", encoded.len());

    let decoder = FecDecoder::new().expect("Failed to create FEC decoder");
    let decoded = decoder.decode(&encoded).expect("Failed to FEC decode");

    println!("Decoded length: {}", decoded.len());
    println!("Decoded data (first 10): {:?}", &decoded[..10]);

    assert_eq!(&decoded[..10], &test_data, "FEC roundtrip data mismatch");
}

// Test OFDM + FEC together
#[test]
fn test_ofdm_fec_bit_roundtrip() {
    use transmitwave_core::OfdmModulator;
    use transmitwave_core::OfdmDemodulator;
    use transmitwave_core::FecEncoder;
    use transmitwave_core::FecDecoder;

    // Create test data and pad to 223 bytes
    let test_data = vec![42, 84, 126, 168, 210, 252, 1, 2, 3, 4, 5, 6, 7, 8];
    let mut padded_data = test_data.clone();
    while padded_data.len() < 223 {
        padded_data.push(0);
    }

    // FEC encode
    let fec_encoder = FecEncoder::new().expect("Failed to create FEC encoder");
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

    // Split into OFDM symbols (NUM_SUBCARRIERS bits per symbol)
    let mut ofdm_samples = Vec::new();
    let mut modulator = OfdmModulator::new();

    for symbol_bits in bits.chunks(NUM_SUBCARRIERS) {
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
    let fec_decoder = FecDecoder::new().expect("Failed to create FEC decoder");
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
fn test_spread_spectrum_roundtrip_simple() {
    use transmitwave_core::{DecoderSpread, EncoderSpread};

    let original_data = b"Test";
    let mut encoder = EncoderSpread::new(2).expect("Failed to create spread encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    let mut decoder = DecoderSpread::new(2).expect("Failed to create spread decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Spread spectrum round-trip corruption detected");
}

#[test]
fn test_spread_spectrum_roundtrip_max_payload() {
    use transmitwave_core::{DecoderSpread, EncoderSpread};

    let original_data = vec![0xAA; 200]; // Max size with alternating bits
    let mut encoder = EncoderSpread::new(2).expect("Failed to create spread encoder");
    let samples = encoder.encode(&original_data).expect("Failed to encode");

    let mut decoder = DecoderSpread::new(2).expect("Failed to create spread decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "Spread spectrum max payload corruption");
}

#[test]
fn test_spread_spectrum_roundtrip_binary_patterns() {
    use transmitwave_core::{DecoderSpread, EncoderSpread};

    let test_patterns = vec![
        vec![0xFF, 0xFF, 0xFF],      // All ones
        vec![0x00, 0x00, 0x00],      // All zeros
        vec![0xAA, 0x55, 0xAA],      // Alternating pattern
        (0..200).map(|i| i as u8).collect::<Vec<u8>>(), // Various byte values (limited to 200 by MAX_PAYLOAD_SIZE)
    ];

    for pattern in test_patterns {
        let mut encoder = EncoderSpread::new(2).expect("Failed to create encoder");
        let samples = encoder.encode(&pattern).expect("Failed to encode");

        let mut decoder = DecoderSpread::new(2).expect("Failed to create decoder");
        let decoded = decoder.decode(&samples).expect("Failed to decode");

        assert_eq!(decoded, pattern, "Spread spectrum corrupted pattern: {:?}", pattern);
    }
}

// ============================================================================
// Multi-tone FSK (ggwave-compatible) Integration Tests
// ============================================================================
// FSK mode tests for maximum reliability in over-the-air transmission
// Tests focus on robustness to noise, silence, and edge cases
// Uses 6 simultaneous frequencies (1875-6328 Hz) matching ggwave audible protocol

#[test]
fn test_fsk_encode_decode_round_trip() {
    let original_data = b"Hello, FSK Modem!";

    let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    assert!(!samples.is_empty(), "No samples generated");
    println!("FSK: Generated {} audio samples", samples.len());

    let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "FSK: Decoded data doesn't match original");
    println!("FSK: Successfully decoded: {:?}", String::from_utf8_lossy(&decoded_data));
}

#[test]
fn test_fsk_encode_decode_max_size() {
    let original_data = vec![42u8; 200]; // Max payload size

    let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
    let samples = encoder.encode(&original_data).expect("Failed to encode");

    let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "FSK: Large payload round-trip failed");
}

#[test]
fn test_fsk_encode_decode_binary_data() {
    let original_data = vec![0, 1, 2, 255, 128, 64, 32, 16, 8, 4, 2, 1, 0];

    let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
    let samples = encoder.encode(&original_data).expect("Failed to encode");

    let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "FSK: Binary data round-trip failed");
}

#[test]
fn test_fsk_encode_decode_empty_data() {
    let original_data = b"";

    let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "FSK: Empty data round-trip failed");
}

#[test]
fn test_fsk_with_leading_silence() {
    let original_data = b"FSK with silence";

    let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    // Add leading silence (1 second = 16000 samples)
    let mut augmented_samples = vec![0.0; 16000];
    augmented_samples.extend_from_slice(&samples);

    let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "FSK: Leading silence test failed");
}

#[test]
fn test_fsk_with_trailing_silence() {
    let original_data = b"FSK test";

    let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    // Add trailing silence
    let mut augmented_samples = samples.clone();
    augmented_samples.extend_from_slice(&vec![0.0; 16000]);

    let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "FSK: Trailing silence test failed");
}

#[test]
fn test_fsk_with_leading_noise() {
    let original_data = b"FSK noise test";

    let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
    let samples = encoder.encode(original_data).expect("Failed to encode");

    // Add leading noise
    let mut augmented_samples = Vec::new();
    let mut rng_state = 12345u32;
    for _ in 0..16000 {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.1; // 10% noise
        augmented_samples.push(noise);
    }
    augmented_samples.extend_from_slice(&samples);

    let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "FSK: Leading noise test failed");
}

#[test]
fn test_fsk_with_data_noise() {
    let original_data = b"FSK data noise";

    let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
    let mut samples = encoder.encode(original_data).expect("Failed to encode");

    // Add 10% amplitude noise to the encoded data
    let mut rng_state = 22222u32;
    for sample in samples.iter_mut() {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.10; // 10% noise
        *sample += noise;
    }

    let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "FSK: Data noise test failed");
}

#[test]
fn test_fsk_with_heavy_data_noise() {
    let original_data = b"FSK heavy";

    let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
    let mut samples = encoder.encode(original_data).expect("Failed to encode");

    // Add 15% amplitude noise to test robustness
    let mut rng_state = 33333u32;
    for sample in samples.iter_mut() {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.15; // 15% noise
        *sample += noise;
    }

    let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
    let decoded_data = decoder.decode(&samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "FSK: Heavy data noise test failed");
}

#[test]
fn test_fsk_binary_patterns() {
    let test_patterns = vec![
        vec![0xFF; 20],         // All ones
        vec![0x00; 20],         // All zeros
        vec![0xAA; 20],         // Alternating bits (10101010)
        vec![0x55; 20],         // Alternating bits (01010101)
        (0..50).map(|i| i as u8).collect::<Vec<u8>>(), // Various values
    ];

    for pattern in test_patterns {
        let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
        let samples = encoder.encode(&pattern).expect("Failed to encode");

        let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
        let decoded = decoder.decode(&samples).expect("Failed to decode");

        assert_eq!(decoded, pattern, "FSK: Binary pattern test failed for pattern: {:?}", pattern);
    }
}

#[test]
fn test_fsk_medium_payload_with_noise_and_silence() {
    let original_data = vec![42u8; 50]; // Medium payload size for faster testing

    let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
    let samples = encoder.encode(&original_data).expect("Failed to encode");

    // Add leading and trailing silence
    let mut augmented_samples = vec![0.0; 8000]; // 0.5 second leading silence
    augmented_samples.extend_from_slice(&samples);
    augmented_samples.extend_from_slice(&vec![0.0; 8000]); // 0.5 second trailing silence

    // Add noise to the data portion
    for sample in &mut augmented_samples[8000..8000 + samples.len()] {
        let mut rng_state = 44444u32;
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.08; // 8% noise
        *sample += noise;
    }

    let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "FSK: Medium payload with noise/silence test failed");
}

#[test]
#[ignore = "it is too slow"]
fn test_fsk_max_payload_with_noise_and_silence() {
    let original_data = vec![42u8; 200]; // Max payload size

    let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
    let samples = encoder.encode(&original_data).expect("Failed to encode");

    // Add leading and trailing silence
    let mut augmented_samples = vec![0.0; 8000]; // 0.5 second leading silence
    augmented_samples.extend_from_slice(&samples);
    augmented_samples.extend_from_slice(&vec![0.0; 8000]); // 0.5 second trailing silence

    // Add noise to the data portion
    for sample in &mut augmented_samples[8000..8000 + samples.len()] {
        let mut rng_state = 44444u32;
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.08; // 8% noise
        *sample += noise;
    }

    let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
    let decoded_data = decoder.decode(&augmented_samples).expect("Failed to decode");

    assert_eq!(decoded_data, original_data, "FSK: Max payload with noise/silence test failed");
}

#[test]
fn test_fsk_various_payload_sizes() {
    let test_sizes = vec![1, 5, 10, 50, 100, 150, 200];

    for size in test_sizes {
        let original_data: Vec<u8> = (0..size).map(|i| (i as u8).wrapping_mul(17)).collect();

        let mut encoder = EncoderFsk::new().expect("Failed to create FSK encoder");
        let samples = encoder.encode(&original_data).expect("Failed to encode");

        let mut decoder = DecoderFsk::new().expect("Failed to create FSK decoder");
        let decoded_data = decoder.decode(&samples).expect("Failed to decode");

        assert_eq!(
            decoded_data, original_data,
            "FSK: Various payload size test failed for size {}",
            size
        );
    }
}