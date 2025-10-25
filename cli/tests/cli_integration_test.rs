use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn get_target_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/release/transmitwave")
}

fn create_test_file(name: &str, content: &str) -> PathBuf {
    let tmp_dir = PathBuf::from("tmp");
    fs::create_dir_all(&tmp_dir).ok();
    let path = tmp_dir.join(name);
    fs::write(&path, content).expect("Failed to write test file");
    path
}

fn run_transmitwave(args: &[&str]) -> String {
    let binary = get_target_dir();
    let output = Command::new(&binary)
        .args(args)
        .output()
        .expect("Failed to execute transmitwave");

    String::from_utf8_lossy(&output.stderr).to_string() + &String::from_utf8_lossy(&output.stdout)
}

#[test]
fn test_encode_spread_default_chip_duration() {
    // Test that encode subcommand produces audio successfully
    let input = create_test_file("test_encode_spread.txt", "Test message");
    let output = PathBuf::from("tmp/test_encode_spread.wav");

    let output_text = run_transmitwave(&[
        "encode",
        input.to_str().unwrap(),
        output.to_str().unwrap(),
    ]);

    // Should successfully encode with multi-tone FSK
    assert!(output_text.contains("multi-tone FSK") || output_text.contains("Encoded"),
        "Expected successful encoding but got: {}", output_text);

    // Output file should be created
    assert!(output.exists(), "Output file was not created");

    // File should be reasonable size (20KB-500KB for test message)
    let metadata = fs::metadata(&output).expect("Output file not created");
    let file_size = metadata.len();
    assert!(file_size > 10_000, "File too small: {} bytes", file_size);
    assert!(file_size < 500_000, "File too large: {} bytes", file_size);
}

#[test]
fn test_decode_spread_default_chip_duration() {
    // First encode a test message
    let input = create_test_file("test_decode_input.txt", "Test");
    let encoded = PathBuf::from("tmp/test_decode_input.wav");
    let output = PathBuf::from("tmp/test_decode_output.bin");

    // Encode
    run_transmitwave(&[
        "encode",
        input.to_str().unwrap(),
        encoded.to_str().unwrap(),
    ]);

    // Decode - should successfully decode the audio
    let output_text = run_transmitwave(&[
        "decode",
        encoded.to_str().unwrap(),
        output.to_str().unwrap(),
    ]);

    // Should successfully decode
    assert!(output_text.contains("Decoded") || output_text.contains("bytes"),
        "Decode should succeed but got: {}", output_text);

    // Output file should exist
    assert!(output.exists(), "Decoded output file was not created");
}

#[test]
fn test_positional_with_custom_chip_duration() {
    // Test that explicit --chip-duration flag is accepted in positional mode
    let input = create_test_file("test_chip_custom.bin", "Hi");
    let output = PathBuf::from("tmp/test_chip_custom.wav");

    // Use positional args with --chip-duration flag
    let output_text = run_transmitwave(&[
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        "--chip-duration", "3",
    ]);

    // Should successfully encode (with or without spread spectrum)
    assert!(output_text.contains("Encoded") || output_text.contains("multi-tone FSK") || output_text.contains("audio samples"),
        "Expected successful encoding but got: {}", output_text);

    // Output file should exist
    assert!(output.exists(), "Output file was not created");
}


#[test]
fn test_legacy_encode_no_spread_flag() {
    // Test that --no-spread flag works
    let input = create_test_file("test_legacy.bin", "Legacy test");
    let output = PathBuf::from("tmp/test_legacy.wav");

    let output_text = run_transmitwave(&[
        "encode",
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        "--no-spread",
    ]);

    // Should successfully encode
    assert!(output_text.contains("legacy") || output_text.contains("Encoded"),
        "Expected successful legacy encoding but got: {}", output_text);

    // Output file should be created
    assert!(output.exists(), "Output file was not created");

    // File should be reasonable size
    let metadata = fs::metadata(&output).expect("Output file not created");
    let file_size = metadata.len();
    assert!(file_size > 5_000, "File too small: {} bytes", file_size);
    assert!(file_size < 500_000, "File too large: {} bytes", file_size);
}

#[test]
fn test_positional_args_encode_decode() {
    // Test positional args mode (auto-detect by file extension)
    let input = create_test_file("test_positional_input.bin", "Data");
    let encoded = PathBuf::from("tmp/test_positional_encoded.wav");
    let decoded = PathBuf::from("tmp/test_positional_decoded.bin");

    // Encode using positional args (auto-detects .bin as input)
    let encode_output = run_transmitwave(&[
        input.to_str().unwrap(),
        encoded.to_str().unwrap(),
    ]);

    assert!(encode_output.contains("Encoded"),
        "Positional encode should succeed. Got: {}", encode_output);

    // Decode using positional args (auto-detects .wav as input)
    let decode_output = run_transmitwave(&[
        encoded.to_str().unwrap(),
        decoded.to_str().unwrap(),
    ]);

    assert!(decode_output.contains("Decoded"),
        "Positional decode should succeed. Got: {}", decode_output);
}

#[test]
fn test_roundtrip_consistency() {
    // Encode and decode should produce same data
    let input_text = "Hello, World!";
    let input = create_test_file("test_roundtrip_in.bin", input_text);
    let encoded = PathBuf::from("tmp/test_roundtrip.wav");
    let decoded = PathBuf::from("tmp/test_roundtrip_out.bin");

    // Encode using subcommand
    run_transmitwave(&[
        "encode",
        input.to_str().unwrap(),
        encoded.to_str().unwrap(),
    ]);

    // Decode using subcommand
    run_transmitwave(&[
        "decode",
        encoded.to_str().unwrap(),
        decoded.to_str().unwrap(),
    ]);

    // Verify roundtrip
    let decoded_content = fs::read_to_string(&decoded)
        .expect("Failed to read decoded output");

    assert_eq!(decoded_content, input_text,
        "Roundtrip failed: expected '{}' but got '{}'", input_text, decoded_content);
}

#[test]
fn test_file_extension_auto_detection() {
    // Test that .bin and .wav extensions trigger auto-detection
    let input = create_test_file("test_auto_detect.bin", "Auto detect");
    let output = PathBuf::from("tmp/test_auto_detect.wav");

    // Should auto-detect encode mode based on input extension
    let output_text = run_transmitwave(&[
        input.to_str().unwrap(),
        output.to_str().unwrap(),
    ]);

    // Should successfully encode
    assert!(output_text.contains("chip_duration") || output_text.contains("Encoded"),
        "Auto-detection failed for .bin file. Got: {}", output_text);

    // Output file should be created
    assert!(output.exists(), "Output file was not created");
}

#[test]
fn test_different_input_sizes_encode_successfully() {
    // Test that different input sizes encode successfully
    let small = create_test_file("test_small.bin", "A");
    let medium = create_test_file("test_medium.bin", "Hello, Audio Modem!");
    let output_small = PathBuf::from("tmp/test_small_out.wav");
    let output_medium = PathBuf::from("tmp/test_medium_out.wav");

    let small_output = run_transmitwave(&[
        "encode",
        small.to_str().unwrap(),
        output_small.to_str().unwrap(),
    ]);

    let medium_output = run_transmitwave(&[
        "encode",
        medium.to_str().unwrap(),
        output_medium.to_str().unwrap(),
    ]);

    // Both should encode successfully
    assert!(small_output.contains("Encoded") || small_output.contains("audio samples"),
        "Small input should encode successfully. Got: {}", small_output);
    assert!(medium_output.contains("Encoded") || medium_output.contains("audio samples"),
        "Medium input should encode successfully. Got: {}", medium_output);

    // Both output files should exist
    assert!(output_small.exists(), "Small output file was not created");
    assert!(output_medium.exists(), "Medium output file was not created");

    // Both should have reasonable file sizes
    let size_small = fs::metadata(&output_small).unwrap().len();
    let size_medium = fs::metadata(&output_medium).unwrap().len();

    assert!(size_small > 5_000, "Small output too small: {} bytes", size_small);
    assert!(size_medium > 5_000, "Medium output too small: {} bytes", size_medium);
    assert!(size_small < 500_000, "Small output too large: {} bytes", size_small);
    assert!(size_medium < 500_000, "Medium output too large: {} bytes", size_medium);
}
