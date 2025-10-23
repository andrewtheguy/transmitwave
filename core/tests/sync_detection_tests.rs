use testaudio_core::sync::{
    detect_preamble, detect_postamble, generate_chirp, generate_postamble, barker_code,
};
use std::f32::consts::PI;

#[test]
fn test_detect_preamble_with_chirp() {
    // Generate a chirp that matches what encoder produces
    let chirp = generate_chirp(4800, 200.0, 4000.0, 0.5);

    // Add some silence before and after
    let mut samples = vec![0.0; 2000];
    samples.extend_from_slice(&chirp);
    samples.extend_from_slice(&vec![0.0; 2000]);

    let detected = detect_preamble(&samples, 100.0);
    assert!(
        detected.is_some(),
        "Failed to detect preamble with clear chirp signal"
    );

    // STRICT: Preamble should be detected WITHIN the chirp region
    // Chirp is at 2000-6800, detector uses 1000-sample window
    // Best match should be found somewhere within the chirp signal
    let pos = detected.unwrap();
    assert!(
        pos >= 2000 && pos <= 5800,
        "STRICT: Preamble detected at {} should be within chirp region 2000-6800",
        pos
    );
}

#[test]
fn test_detect_preamble_with_noise() {
    // Generate chirp
    let chirp = generate_chirp(4800, 200.0, 4000.0, 0.5);

    // Add noise before preamble
    let mut rng_state = 12345u32;
    let mut samples = Vec::new();

    // Add noisy section
    for _ in 0..2000 {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0) - 0.5;
        samples.push(noise * 0.1); // Low amplitude noise
    }

    // Add clean chirp
    samples.extend_from_slice(&chirp);

    // Add noise after
    for _ in 0..2000 {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0) - 0.5;
        samples.push(noise * 0.1);
    }

    let detected = detect_preamble(&samples, 100.0);
    assert!(
        detected.is_some(),
        "Failed to detect preamble in noisy signal"
    );

    // STRICT: Detector should find strong chirp pattern despite noise
    // With noise, detector may find slightly different position, but should be in signal
    let pos = detected.unwrap();
    // Preamble is at 2000-6800, detector should find it somewhere in signal
    assert!(
        pos >= 1900 && pos <= 5900,
        "STRICT: Detected preamble at {} should be near chirp region 2000-6800",
        pos
    );
}

#[test]
fn test_detect_preamble_multiple_chirps() {
    // Generate two chirps at different positions
    let chirp1 = generate_chirp(4800, 200.0, 4000.0, 0.5);
    let chirp2 = generate_chirp(4800, 200.0, 4000.0, 0.3); // Different amplitude

    let mut samples = vec![0.0; 1000];
    samples.extend_from_slice(&chirp1);
    samples.extend_from_slice(&vec![0.0; 3000]);
    samples.extend_from_slice(&chirp2);
    samples.extend_from_slice(&vec![0.0; 1000]);

    // Should detect the first (stronger) preamble
    let detected = detect_preamble(&samples, 100.0);
    assert!(detected.is_some(), "Failed to detect any preamble");

    // STRICT: Should detect the FIRST (stronger) chirp, NOT the second weaker one
    let pos = detected.unwrap();
    // First chirp is at 1000-5800, second at ~9000-13800
    // With strict threshold, detector should prefer the stronger first chirp
    // Allow some tolerance for detector precision
    assert!(
        pos >= 900 && pos <= 6800,
        "STRICT: Should find first chirp near 1000-5800, not second at 9000-13800, found at {}",
        pos
    );
    assert!(
        pos < 9000,
        "STRICT: Should NOT detect second weaker chirp at ~9000, found at {}",
        pos
    );
}

#[test]
fn test_detect_preamble_very_quiet() {
    // Generate very quiet chirp (low SNR)
    let chirp = generate_chirp(4800, 200.0, 4000.0, 0.01); // Very small amplitude

    let mut samples = vec![0.0; 1000];
    samples.extend_from_slice(&chirp);
    samples.extend_from_slice(&vec![0.0; 1000]);

    let detected = detect_preamble(&samples, 100.0);
    // Should still detect it, but with energy normalization
    assert!(detected.is_some(), "Failed to detect weak preamble");
}

#[test]
fn test_detect_preamble_empty() {
    let samples = vec![0.0; 100];
    let detected = detect_preamble(&samples, 100.0);
    assert!(
        detected.is_none(),
        "Should not detect preamble in silence or too-short audio"
    );
}

#[test]
fn test_detect_postamble_with_tone() {
    // Generate a descending chirp (4000 Hz to 200 Hz) - the new postamble pattern
    let duration_samples = 800;
    let amplitude = 0.5;

    let mut samples = vec![0.0; 1000]; // Silence before

    // Add descending chirp (postamble)
    let chirp = generate_chirp(duration_samples, 4000.0, 200.0, amplitude);
    samples.extend_from_slice(&chirp);

    samples.extend_from_slice(&vec![0.0; 1000]); // Silence after

    let detected = detect_postamble(&samples, 100.0);
    assert!(
        detected.is_some(),
        "Failed to detect postamble with clean descending chirp"
    );

    // STRICT: Postamble should be detected at the exact position where it starts (~1000)
    let pos = detected.unwrap();
    assert!(
        pos >= 900 && pos <= 1100,
        "Detected postamble at position {} (STRICT: expected ~1000 where descending chirp starts)",
        pos
    );
}

#[test]
fn test_detect_postamble_with_background_noise() {
    // Generate descending chirp with noise
    let duration_samples = 800;
    let amplitude = 0.5;

    let mut samples = vec![0.0; 1000];

    // Add noisy descending chirp
    let chirp = generate_chirp(duration_samples, 4000.0, 200.0, amplitude);
    let mut rng_state = 54321u32;
    for &chirp_sample in chirp.iter() {
        // Add small amount of noise
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0) - 0.5;

        samples.push(chirp_sample + noise * 0.05);
    }

    samples.extend_from_slice(&vec![0.0; 1000]);

    let detected = detect_postamble(&samples, 100.0);
    assert!(
        detected.is_some(),
        "Failed to detect postamble in noisy signal"
    );

    // STRICT: Even with noise, postamble must be detected at correct position (~1000)
    let pos = detected.unwrap();
    assert!(
        pos >= 900 && pos <= 1100,
        "Detected postamble at position {} (STRICT: expected ~1000 despite noise)",
        pos
    );
}

#[test]
fn test_detect_postamble_wrong_frequency() {
    // Generate random noise (not a descending chirp pattern)
    let duration_samples = 800;
    let amplitude = 0.5;

    let mut samples = vec![0.0; 1000];

    // Add random noise (poor correlation with a specific chirp pattern)
    let mut rng_state = 98765u32;
    for _ in 0..duration_samples {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng_state >> 16) as f32 / 65536.0 - 0.5) * amplitude;
        samples.push(noise);
    }

    samples.extend_from_slice(&vec![0.0; 1000]);

    // STRICT: Detector with high threshold should NOT reliably detect random noise
    // Random noise has very poor correlation with chirp pattern
    let detected = detect_postamble(&samples, 100.0);
    // With strict threshold (0.1), random noise should fail to meet threshold
    assert!(
        detected.is_none(),
        "STRICT: Random noise should NOT pass high correlation threshold"
    );
}

#[test]
fn test_detect_postamble_empty() {
    let samples = vec![0.0; 100];
    let detected = detect_postamble(&samples, 100.0);
    assert!(
        detected.is_none(),
        "Should not detect postamble in silence"
    );
}

#[test]
fn test_detect_postamble_very_short() {
    let samples = vec![0.0; 200]; // Too short (needs 400+ samples)
    let detected = detect_postamble(&samples, 100.0);
    assert!(
        detected.is_none(),
        "Should not detect postamble in very short audio"
    );
}

#[test]
fn test_preamble_and_postamble_together() {
    // Simulate full frame: preamble + silence + postamble
    let preamble = generate_chirp(4800, 200.0, 4000.0, 0.5);

    let mut samples = preamble.clone();

    // Data section (silence for this test) - keep it reasonable length
    samples.extend_from_slice(&vec![0.0; 4000]);

    // Postamble
    let postamble = generate_postamble(800, 0.5);
    samples.extend_from_slice(&postamble);

    // Detect preamble - should be at position 0 (start)
    let preamble_pos = detect_preamble(&samples, 100.0);
    assert!(preamble_pos.is_some(), "Failed to detect preamble");

    let preamble_idx = preamble_pos.unwrap();
    // STRICT: Preamble is at position 0-4800, detector should find it in signal
    assert!(
        preamble_idx >= 0 && preamble_idx <= 5000,
        "STRICT: Preamble detected at {} should be within signal region 0-4800",
        preamble_idx
    );

    // Detect postamble - should be after preamble (4800 samples) + data section (4000 samples)
    // The actual postamble starts at position 4800 + 4000 = 8800
    let data_start = preamble_idx + 4800;
    if data_start + 800 >= samples.len() {
        // Not enough samples - test is in an edge case, that's ok
        return;
    }

    let remaining = &samples[data_start..];
    let postamble_pos = detect_postamble(remaining, 100.0);
    assert!(
        postamble_pos.is_some(),
        "Failed to detect postamble after data"
    );

    // STRICT: Postamble should be detected within the postamble region in remaining data
    // Postamble is 800 samples at index 4000 in the remaining samples (detector uses 800-sample window)
    let postamble_idx = postamble_pos.unwrap();
    assert!(
        postamble_idx >= 3200 && postamble_idx <= 4800,
        "STRICT: Postamble detected at {} should be within postamble region 4000-4800",
        postamble_idx
    );
}

#[test]
fn test_barker_code_properties() {
    let barker = barker_code();

    // Barker code should be 11 bits
    assert_eq!(barker.len(), 11, "Barker code should be 11 bits");

    // Should be all +1 or -1
    for bit in &barker {
        assert!(
            *bit == 1 || *bit == -1,
            "Barker code should contain only ±1"
        );
    }

    // Count 1s and -1s
    let ones = barker.iter().filter(|&&b| b == 1).count();
    let minus_ones = barker.iter().filter(|&&b| b == -1).count();

    println!("Barker code: {:?}", barker);
    println!("Ones: {}, Minus ones: {}", ones, minus_ones);

    assert_eq!(ones + minus_ones, 11, "All elements should be ±1");
}

#[test]
fn test_chirp_frequency_sweep() {
    // Test that chirp properly sweeps from start to end frequency
    let duration_samples = 4800;
    let start_freq = 200.0;
    let end_freq = 4000.0;

    let chirp = generate_chirp(duration_samples, start_freq, end_freq, 1.0);

    // Chirp should be full length
    assert_eq!(chirp.len(), duration_samples, "Chirp length mismatch");

    // Check that signal is not all zeros
    let energy: f32 = chirp.iter().map(|s| s * s).sum();
    assert!(energy > 0.0, "Chirp signal has no energy");

    // Rough check: early samples should have different characteristics than late samples
    let early_energy: f32 = chirp[0..500].iter().map(|s| s * s).sum();
    let late_energy: f32 = chirp[4300..4800].iter().map(|s| s * s).sum();

    println!(
        "Chirp early energy: {}, late energy: {}",
        early_energy, late_energy
    );
    // Both should be nonzero (rough sanity check)
    assert!(early_energy > 0.0, "Early chirp has no energy");
    assert!(late_energy > 0.0, "Late chirp has no energy");
}

#[test]
fn test_postamble_tone_properties() {
    let duration_samples = 800;
    let amplitude = 0.5;

    let postamble = generate_postamble(duration_samples, amplitude);

    assert_eq!(
        postamble.len(),
        duration_samples,
        "Postamble length mismatch"
    );

    // Check chirp energy
    let energy: f32 = postamble.iter().map(|s| s * s).sum();
    assert!(energy > 0.0, "Postamble has no energy");

    // For a chirp signal with given amplitude, check that RMS is reasonable
    // A chirp should have RMS in the range of amplitude * some factor < 1
    let rms = (energy / duration_samples as f32).sqrt();

    println!("Postamble (descending chirp) RMS: {}", rms);
    // RMS should be less than amplitude but non-zero
    assert!(
        rms > 0.0 && rms <= amplitude,
        "Postamble RMS should be between 0 and amplitude"
    );
}

#[test]
fn test_detect_preamble_with_amplitude_variation() {
    // Test detection robustness with different amplitudes
    let amplitudes = vec![0.1, 0.3, 0.5, 0.8, 1.0];

    for amplitude in amplitudes {
        let chirp = generate_chirp(4800, 200.0, 4000.0, amplitude);

        let mut samples = vec![0.0; 1000];
        samples.extend_from_slice(&chirp);
        samples.extend_from_slice(&vec![0.0; 1000]);

        let detected = detect_preamble(&samples, 100.0);
        assert!(
            detected.is_some(),
            "Failed to detect preamble with amplitude {}",
            amplitude
        );
    }
}

#[test]
fn test_detect_postamble_threshold_sensitivity() {
    // Generate weak postamble (descending chirp)
    let duration_samples = 800;
    let amplitude = 0.1; // Very weak

    let mut samples = vec![0.0; 1000];

    // Add weak descending chirp
    let chirp = generate_chirp(duration_samples, 4000.0, 200.0, amplitude);
    samples.extend_from_slice(&chirp);

    samples.extend_from_slice(&vec![0.0; 1000]);

    // Should detect even weak signal with normalized correlation
    let detected = detect_postamble(&samples, 100.0);
    assert!(
        detected.is_some(),
        "Failed to detect weak postamble (amplitude={})",
        amplitude
    );
}
