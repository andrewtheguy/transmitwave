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
