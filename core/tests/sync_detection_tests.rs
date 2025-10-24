use testaudio_core::sync::{
    detect_preamble, detect_postamble, generate_chirp, generate_postamble,
    generate_preamble_noise, generate_postamble_noise, barker_code,
};
use testaudio_core::{PREAMBLE_SAMPLES, POSTAMBLE_SAMPLES, fft_correlate_1d, Mode};
use rand::SeedableRng;
use rand_distr::Normal;

// Signal generation helpers - signal-agnostic factory functions
// These generate the same signals used by the actual detector implementations
fn create_test_preamble(amplitude: f32) -> Vec<f32> {
    generate_preamble_noise(PREAMBLE_SAMPLES, amplitude)
}

fn create_test_postamble(amplitude: f32) -> Vec<f32> {
    generate_postamble_noise(POSTAMBLE_SAMPLES, amplitude)
}

#[test]
fn test_detect_preamble_with_chirp() {
    // Generate preamble using agnostic helper
    let preamble = create_test_preamble(0.5);

    // Add silence before and after
    let mut samples = vec![0.0; 4000];
    samples.extend_from_slice(&preamble);
    samples.extend_from_slice(&vec![0.0; 4000]);

    let detected = detect_preamble(&samples, 100.0);
    assert!(
        detected.is_some(),
        "Failed to detect preamble with clear signal"
    );

    // STRICT: Preamble should be detected exactly at the start position
    let pos = detected.unwrap();
    assert_eq!(
        pos, 4000,
        "STRICT: Preamble must be detected at exact position 4000 (silence + start of preamble)"
    );
}

#[test]
fn test_detect_postamble_with_chirp() {
    // Generate postamble using agnostic helper
    let postamble = create_test_postamble(0.5);

    // Add silence before and after
    let mut samples = vec![0.0; 4000];
    samples.extend_from_slice(&postamble);
    samples.extend_from_slice(&vec![0.0; 4000]);

    let detected = detect_postamble(&samples, 100.0);
    assert!(
        detected.is_some(),
        "Failed to detect postamble with clear signal"
    );

    // STRICT: Postamble should be detected exactly at the start position
    let pos = detected.unwrap();
    assert_eq!(
        pos, 4000,
        "STRICT: Postamble must be detected at exact position 4000 (silence + start of postamble)"
    );
}

#[test]
fn test_detect_preamble_with_noise() {
    // Generate preamble using agnostic helper
    let preamble = create_test_preamble(0.5);

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
    // Generate postamble using agnostic helper
    let postamble = create_test_postamble(0.5);

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

    // Add preamble using agnostic helper
    let preamble = create_test_preamble(0.5);
    samples.extend_from_slice(&preamble);

    // Add data section (silence, simulating OFDM data)
    samples.extend_from_slice(&vec![0.0; 8000]);

    // Add postamble using agnostic helper
    let postamble = create_test_postamble(0.5);
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

#[test]
fn test_fft_correlation_index_mapping_with_preamble() {
    // Comment 10: Integration test ensuring sync.rs uses the correct FFT index mapping (i + L-1)
    // Build a short synthetic signal where a template is inserted at known position i0

    let insert_pos = 500;  // Insert template at this position in signal

    // Create template using agnostic helper
    let template = create_test_preamble(0.5);
    let template_len = template.len();

    // Create signal with silence before and after
    let mut signal = vec![0.0; insert_pos];
    signal.extend_from_slice(&template);
    signal.extend_from_slice(&vec![0.0; 1000]);

    // Compute FFT correlation
    let fft_result = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();

    // Find peak index
    let peak_idx = fft_result.iter()
        .enumerate()
        .filter(|(_, value)| !value.is_nan())
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .expect("No valid peak found: all values were NaN");

    // According to index mapping: window at position i maps to output index i + template_len - 1
    // So if template inserted at position insert_pos, peak should be at insert_pos + template_len - 1
    let expected_peak_idx = insert_pos + template_len - 1;

    assert_eq!(peak_idx, expected_peak_idx,
        "FFT correlation peak should be at i + L - 1 = {} + {} - 1 = {}, got {}",
        insert_pos, template_len, expected_peak_idx, peak_idx);

    // Now verify that detect_preamble returns the correct window start position
    let detected = detect_preamble(&signal, 100.0);

    assert!(detected.is_some(), "Preamble should be detected");
    let detected_pos = detected.unwrap();

    // The detection should find the start of the template
    assert_eq!(detected_pos, insert_pos,
        "Preamble should be detected at window start position {}, got {}",
        insert_pos, detected_pos);
}

#[test]
fn test_fft_correlation_index_mapping_with_preamble_noisy() {
    // Comment 11: Noisy variant of the FFT index mapping test
    // Verify that despite Gaussian noise, FFT peak and detection still map correctly

    let insert_pos = 500;

    // Create clean template using agnostic helper
    let template = create_test_preamble(0.5);
    let template_len = template.len();

    // Compute template RMS for SNR-relative noise scaling
    let template_rms: f32 = (template.iter().map(|s| s * s).sum::<f32>() / template.len() as f32).sqrt();

    // Create signal with template at known position
    let mut signal = vec![0.0; insert_pos];
    signal.extend_from_slice(&template);
    signal.extend_from_slice(&vec![0.0; 1000]);

    // Add Gaussian noise scaled to 20% of template RMS (SNR ≈ 13.98 dB)
    let noise_std = template_rms * 0.2;
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let normal = Normal::new(0.0, noise_std).expect("Failed to create normal distribution");
    use rand::distributions::Distribution;

    for sample in &mut signal {
        *sample += normal.sample(&mut rng);
    }

    // Verify: FFT correlation peak should still be at insert_pos + template_len - 1
    let fft_result = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();

    let peak_idx = fft_result.iter()
        .enumerate()
        .filter(|(_, value)| !value.is_nan())
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .expect("No valid peak found in noisy signal");

    let expected_peak_idx = insert_pos + template_len - 1;

    // Allow ±1 sample tolerance for noisy conditions
    let tolerance = 1;
    assert!(
        (peak_idx as i32 - expected_peak_idx as i32).abs() <= tolerance,
        "FFT correlation peak with noise should be near i + L - 1 = {} + {} - 1 = {}, got {}",
        insert_pos, template_len, expected_peak_idx, peak_idx
    );

    // Verify: detect_preamble should still find the template start (±1 sample tolerance)
    let detected = detect_preamble(&signal, 100.0);

    assert!(detected.is_some(), "Preamble should be detected in noisy signal");
    let detected_pos = detected.unwrap();

    // Allow ±1 sample tolerance for detection in noisy conditions
    assert!(
        (detected_pos as i32 - insert_pos as i32).abs() <= tolerance,
        "Preamble in noisy signal should be detected near window start position {}, got {}",
        insert_pos, detected_pos
    );
}
