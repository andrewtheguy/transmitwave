// ============================================================================
// INTEGRATION TESTS - FSK MODE ONLY
// ============================================================================
// These tests perform full encode/decode roundtrips with preamble/postamble
// synchronization using multi-tone FSK modulation.
//
// For faster test execution, run in release mode:
//   cargo test -p transmitwave-core --test integration_test --release
//
// The slowness is inherent to preamble/postamble detection, not a bug.
// ============================================================================

use transmitwave_core::{EncoderFsk, DecoderFsk};

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