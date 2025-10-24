use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn get_target_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/release/testaudio")
}

fn create_test_file(name: &str, content: &str) -> PathBuf {
    let tmp_dir = PathBuf::from("tmp");
    fs::create_dir_all(&tmp_dir).ok();
    let path = tmp_dir.join(name);
    fs::write(&path, content).expect("Failed to write test file");
    path
}

fn run_testaudio(args: &[&str]) -> String {
    let binary = get_target_dir();
    let output = Command::new(&binary)
        .args(args)
        .output()
        .expect("Failed to execute testaudio");

    String::from_utf8_lossy(&output.stderr).to_string() + &String::from_utf8_lossy(&output.stdout)
}

#[test]
fn test_encode_spread_default_chip_duration() {
    // Test that encode subcommand uses correct default chip_duration (2, not 48)
    let input = create_test_file("test_encode_spread.txt", "Test message");
    let output = PathBuf::from("tmp/test_encode_spread.wav");

    let output_text = run_testaudio(&[
        "encode",
        input.to_str().unwrap(),
        output.to_str().unwrap(),
    ]);

    // Should use chip_duration=2, not chunk_bits=48
    assert!(output_text.contains("chip_duration=2"),
        "Expected chip_duration=2 but got: {}", output_text);

    // Should produce ~141,600 samples (8.85 seconds) not 3.3M samples
    assert!(output_text.contains("141600") || output_text.contains("141,600"),
        "Expected ~141,600 samples but got: {}", output_text);

    // File should be ~280KB not 6.3MB
    let metadata = fs::metadata(&output).expect("Output file not created");
    let file_size = metadata.len();
    assert!(file_size < 500_000, "File too large: {} bytes (expected ~280KB)", file_size);
    assert!(file_size > 200_000, "File too small: {} bytes (expected ~280KB)", file_size);
}

#[test]
fn test_decode_spread_default_chip_duration() {
    // First encode a test message
    let input = create_test_file("test_decode_input.txt", "Test");
    let encoded = PathBuf::from("tmp/test_decode_input.wav");
    let output = PathBuf::from("tmp/test_decode_output.bin");

    // Encode
    run_testaudio(&[
        "encode",
        input.to_str().unwrap(),
        encoded.to_str().unwrap(),
    ]);

    // Decode - should use chip_duration from cli.chip_duration, not chunk_bits
    let output_text = run_testaudio(&[
        "decode",
        encoded.to_str().unwrap(),
        output.to_str().unwrap(),
    ]);

    assert!(output_text.contains("chip_duration=2"),
        "Decode should use chip_duration=2 but got: {}", output_text);
}

#[test]
fn test_positional_with_custom_chip_duration() {
    // Test that explicit --chip-duration flag is respected in positional mode
    let input = create_test_file("test_chip_custom.bin", "Hi");
    let output = PathBuf::from("tmp/test_chip_custom.wav");

    // Use positional args with --chip-duration flag
    let output_text = run_testaudio(&[
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        "--chip-duration", "3",
    ]);

    // Should use chip_duration=3 (larger file than default chip_duration=2)
    assert!(output_text.contains("chip_duration=3"),
        "Expected chip_duration=3 but got: {}", output_text);

    // Should produce ~210,400 samples (13.15 seconds) = 141,600 * 3/2
    // (scaling by chip_duration factor: 141600 samples at chip_duration=2 → 210400 at chip_duration=3)
    assert!(output_text.contains("210400") || output_text.contains("210,400"),
        "Expected ~210,400 samples but got: {}", output_text);
}

#[test]
fn test_encode_subcommand_chunk_bits_param() {
    // Verify that chunk_bits doesn't interfere with spread spectrum
    // chunk_bits=64 should NOT result in chip_duration=64
    let input = create_test_file("test_chunk_bits.txt", "Data");
    let output = PathBuf::from("tmp/test_chunk_bits.wav");

    let output_text = run_testaudio(&[
        "encode",
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        "--chunk-bits", "64",
    ]);

    // Should still use chip_duration=2 (not chunk_bits=64)
    assert!(output_text.contains("chip_duration=2"),
        "chunk_bits should not affect chip_duration. Got: {}", output_text);

    // Should NOT produce huge file from 64x spreading
    let metadata = fs::metadata(&output).expect("Output file not created");
    let file_size = metadata.len();
    assert!(file_size < 500_000,
        "chunk_bits=64 incorrectly influenced chip_duration, file too large: {} bytes", file_size);
}

#[test]
fn test_legacy_encode_no_spread_flag() {
    // Test that --no-spread flag bypasses spread spectrum
    let input = create_test_file("test_legacy.bin", "Legacy test");
    let output = PathBuf::from("tmp/test_legacy.wav");

    let output_text = run_testaudio(&[
        "encode",
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        "--no-spread",
    ]);

    // Should use legacy encoder (no spreading)
    assert!(output_text.contains("legacy"),
        "Expected legacy encoder output but got: {}", output_text);

    // Should produce fewer samples than spread (no chip_duration expansion)
    // Legacy produces ~72,800 samples vs spread's ~141,600
    assert!(output_text.contains("72800") || output_text.contains("72,800"),
        "Legacy encoder should produce ~72,800 samples but got: {}", output_text);
}

#[test]
fn test_positional_args_encode_decode() {
    // Test positional args mode (auto-detect by file extension)
    let input = create_test_file("test_positional_input.bin", "Data");
    let encoded = PathBuf::from("tmp/test_positional_encoded.wav");
    let decoded = PathBuf::from("tmp/test_positional_decoded.bin");

    // Encode using positional args (auto-detects .bin as input)
    let encode_output = run_testaudio(&[
        input.to_str().unwrap(),
        encoded.to_str().unwrap(),
    ]);

    assert!(encode_output.contains("chip_duration=2"),
        "Positional encode should use default chip_duration=2. Got: {}", encode_output);

    // Decode using positional args (auto-detects .wav as input)
    let decode_output = run_testaudio(&[
        encoded.to_str().unwrap(),
        decoded.to_str().unwrap(),
    ]);

    assert!(decode_output.contains("chip_duration=2"),
        "Positional decode should use default chip_duration=2. Got: {}", decode_output);
}

#[test]
fn test_roundtrip_consistency() {
    // Encode and decode should produce same data
    let input_text = "Hello, World!";
    let input = create_test_file("test_roundtrip_in.bin", input_text);
    let encoded = PathBuf::from("tmp/test_roundtrip.wav");
    let decoded = PathBuf::from("tmp/test_roundtrip_out.bin");

    // Encode using subcommand
    run_testaudio(&[
        "encode",
        input.to_str().unwrap(),
        encoded.to_str().unwrap(),
    ]);

    // Decode using subcommand
    run_testaudio(&[
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
    let output_text = run_testaudio(&[
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
fn test_different_input_sizes_same_output_size() {
    // Due to frame/FEC structure, different small inputs produce same output size
    let small = create_test_file("test_small.bin", "A");
    let medium = create_test_file("test_medium.bin", "Hello, Audio Modem!");
    let output_small = PathBuf::from("tmp/test_small_out.wav");
    let output_medium = PathBuf::from("tmp/test_medium_out.wav");

    run_testaudio(&[
        "encode",
        small.to_str().unwrap(),
        output_small.to_str().unwrap(),
    ]);

    run_testaudio(&[
        "encode",
        medium.to_str().unwrap(),
        output_medium.to_str().unwrap(),
    ]);

    // Both should produce similar size due to frame padding
    let size_small = fs::metadata(&output_small).unwrap().len();
    let size_medium = fs::metadata(&output_medium).unwrap().len();

    // Should be within 1KB of each other (frame overhead is constant for small messages)
    let diff = (size_small as i64 - size_medium as i64).abs();
    assert!(diff < 1000,
        "File sizes should be similar for small payloads: {} vs {} (diff: {})",
        size_small, size_medium, diff);
}
