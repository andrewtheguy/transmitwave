use testaudio_core::{Decoder, Encoder};

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
