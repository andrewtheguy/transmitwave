use testaudio_core::sync::{
    detect_preamble, detect_postamble, generate_chirp, generate_postamble, barker_code,
};
use testaudio_core::{PREAMBLE_SAMPLES, POSTAMBLE_SAMPLES};

#[test]
fn test_detect_preamble_with_chirp() {
    // Generate preamble (ascending chirp, 1 second)
    let preamble = generate_chirp(PREAMBLE_SAMPLES, 200.0, 4000.0, 0.5);

    // Add silence before and after
    let mut samples = vec![0.0; 4000];
    samples.extend_from_slice(&preamble);
    samples.extend_from_slice(&vec![0.0; 4000]);

    let detected = detect_preamble(&samples, 100.0);
    assert!(
        detected.is_some(),
        "Failed to detect preamble with clear chirp signal"
    );

    // STRICT: Preamble should be detected exactly at the start position
    let pos = detected.unwrap();
    assert_eq!(
        pos, 4000,
        "STRICT: Preamble must be detected at exact position 4000 (silence + start of chirp)"
    );
}

#[test]
fn test_detect_postamble_with_chirp() {
    // Generate postamble (descending chirp, 1 second)
    let postamble = generate_postamble(POSTAMBLE_SAMPLES, 0.5);

    // Add silence before and after
    let mut samples = vec![0.0; 4000];
    samples.extend_from_slice(&postamble);
    samples.extend_from_slice(&vec![0.0; 4000]);

    let detected = detect_postamble(&samples, 100.0);
    assert!(
        detected.is_some(),
        "Failed to detect postamble with clear chirp signal"
    );

    // STRICT: Postamble should be detected exactly at the start position
    let pos = detected.unwrap();
    assert_eq!(
        pos, 4000,
        "STRICT: Postamble must be detected at exact position 4000 (silence + start of descending chirp)"
    );
}

#[test]
fn test_detect_preamble_with_noise() {
    // Generate preamble
    let preamble = generate_chirp(PREAMBLE_SAMPLES, 200.0, 4000.0, 0.5);

    // Add noise before preamble
    let mut rng_state = 12345u32;
    let mut samples = Vec::new();

    // Add noisy section
    for _ in 0..4000 {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0) - 0.5;
        samples.push(noise * 0.1); // Low amplitude noise
    }

    // Add clean preamble
    samples.extend_from_slice(&preamble);

    // Add noise after
    for _ in 0..4000 {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0) - 0.5;
        samples.push(noise * 0.1);
    }

    let detected = detect_preamble(&samples, 100.0);
    assert!(
        detected.is_some(),
        "Failed to detect preamble in noisy signal"
    );

    // STRICT: Even with noise, detector should find the preamble at correct position
    let pos = detected.unwrap();
    assert_eq!(
        pos, 4000,
        "STRICT: Preamble must be detected at exact position 4000 despite noise"
    );
}

#[test]
fn test_detect_postamble_with_noise() {
    // Generate postamble
    let postamble = generate_postamble(POSTAMBLE_SAMPLES, 0.5);

    // Add noise before and after
    let mut rng_state = 12345u32;
    let mut samples = Vec::new();

    for _ in 0..4000 {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0) - 0.5;
        samples.push(noise * 0.1);
    }

    samples.extend_from_slice(&postamble);

    for _ in 0..4000 {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0) - 0.5;
        samples.push(noise * 0.1);
    }

    let detected = detect_postamble(&samples, 100.0);
    assert!(
        detected.is_some(),
        "Failed to detect postamble in noisy signal"
    );

    // STRICT: Postamble should be detected at exact position despite noise
    let pos = detected.unwrap();
    assert_eq!(
        pos, 4000,
        "STRICT: Postamble must be detected at exact position 4000 despite noise"
    );
}

#[test]
fn test_detect_preamble_wrong_signal() {
    // Generate random noise (not a chirp)
    let mut rng_state = 98765u32;
    let mut samples = vec![0.0; 4000];

    for _ in 0..PREAMBLE_SAMPLES {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.5;
        samples.push(noise);
    }

    samples.extend_from_slice(&vec![0.0; 4000]);

    // STRICT: Should NOT detect random noise as preamble
    let detected = detect_preamble(&samples, 100.0);
    assert!(
        detected.is_none(),
        "STRICT: Random noise should NOT be detected as preamble with high correlation threshold"
    );
}

#[test]
fn test_detect_postamble_wrong_signal() {
    // Generate random noise (not a chirp)
    let mut rng_state = 98765u32;
    let mut samples = vec![0.0; 4000];

    for _ in 0..POSTAMBLE_SAMPLES {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * 0.5;
        samples.push(noise);
    }

    samples.extend_from_slice(&vec![0.0; 4000]);

    // STRICT: Should NOT detect random noise as postamble
    let detected = detect_postamble(&samples, 100.0);
    assert!(
        detected.is_none(),
        "STRICT: Random noise should NOT be detected as postamble with high correlation threshold"
    );
}

#[test]
fn test_full_frame_detection() {
    // Build a complete frame: silence + preamble + data + postamble + silence
    let mut samples = vec![0.0; 2000]; // Initial silence

    // Add preamble
    let preamble = generate_chirp(PREAMBLE_SAMPLES, 200.0, 4000.0, 0.5);
    samples.extend_from_slice(&preamble);

    // Add data section (silence, simulating OFDM data)
    samples.extend_from_slice(&vec![0.0; 8000]);

    // Add postamble
    let postamble = generate_postamble(POSTAMBLE_SAMPLES, 0.5);
    samples.extend_from_slice(&postamble);

    // Add trailing silence
    samples.extend_from_slice(&vec![0.0; 2000]);

    // Detect preamble
    let preamble_pos = detect_preamble(&samples, 100.0);
    assert!(preamble_pos.is_some(), "Failed to detect preamble in full frame");

    let preamble_idx = preamble_pos.unwrap();
    assert_eq!(
        preamble_idx, 2000,
        "STRICT: Preamble must start at position 2000"
    );

    // Detect postamble
    let postamble_pos = detect_postamble(&samples, 100.0);
    assert!(postamble_pos.is_some(), "Failed to detect postamble in full frame");

    let postamble_idx = postamble_pos.unwrap();
    let expected_postamble_pos = 2000 + PREAMBLE_SAMPLES + 8000;
    assert_eq!(
        postamble_idx, expected_postamble_pos,
        "STRICT: Postamble must start at position {} (preamble + data)",
        expected_postamble_pos
    );
}

#[test]
fn test_barker_code_properties() {
    let barker = barker_code();
    assert_eq!(barker.len(), 11, "Barker code should be 11 bits");

    for bit in &barker {
        assert!(
            *bit == 1 || *bit == -1,
            "Barker code should contain only ±1"
        );
    }
}

#[test]
fn test_chirp_generation() {
    let chirp = generate_chirp(16000, 200.0, 4000.0, 1.0);
    assert_eq!(chirp.len(), 16000, "Chirp length should match requested samples");

    let energy: f32 = chirp.iter().map(|s| s * s).sum();
    assert!(energy > 0.0, "Chirp should have non-zero energy");
}

#[test]
fn test_postamble_generation() {
    let postamble = generate_postamble(16000, 0.5);
    assert_eq!(
        postamble.len(),
        16000,
        "Postamble length should match POSTAMBLE_SAMPLES"
    );

    let energy: f32 = postamble.iter().map(|s| s * s).sum();
    assert!(energy > 0.0, "Postamble should have non-zero energy");
}
