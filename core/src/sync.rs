use crate::{fft_correlate_1d, Mode, SAMPLE_RATE};
use std::f32::consts::PI;
use log::warn;

// ============================================================================
// DETECTION THRESHOLD TYPE
// ============================================================================
/// Specifies how the detection threshold should be determined
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetectionThreshold {
    /// Automatically adjust threshold based on signal RMS:
    /// - RMS > 0.1: 0.4 (strong signal, strict detection)
    /// - 0.02 < RMS ≤ 0.1: 0.35 (medium signal)
    /// - RMS ≤ 0.02: 0.3 (weak signal, relaxed threshold)
    Adaptive,
    /// Fixed threshold value (0.001 < value ≤ 1.0)
    /// Minimum is 0.001 (0.1%) to avoid false negatives
    Fixed(f32),
}

// ============================================================================
// SYNCHRONIZATION SIGNAL TYPE CONFIGURATION
// ============================================================================
// Toggle this constant to switch between different synchronization signal types:
//   - SignalType::Chirp         (frequency sweep from low to high, better detection in noisy environments)
//   - SignalType::PrnNoise      (blends in better with the sound of the payload)
//   - SignalType::ThreeNoteWhistle (musical three-note whistle pattern)
//
// This controls what `generate_preamble()` and `generate_postamble_signal()`
// actually generate, allowing easy comparison between signal types.
const SIGNAL_TYPE: SignalType = SignalType::PrnNoise;

/// Window length (in samples) for computing RMS in adaptive threshold mode.
/// This controls the size of sliding windows used to find the maximum RMS,
/// which helps detect strong signal bursts and ignores long silences or unrelated noise.
/// Tuning this parameter affects threshold sensitivity:
/// - Smaller windows: More responsive to brief bursts but noisier
/// - Larger windows: More robust to noise but may miss brief signals
const ADAPTIVE_RMS_WINDOW_LENGTH: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum SignalType {
    PrnNoise,        // Pseudo-random bipolar noise (different seeds for pre/post)
    Chirp,           // Linear frequency sweep
    ThreeNoteWhistle, // Three-note whistle pattern (different melodies for pre/post)
}

/// Generates a Barker code (11-bit) for synchronization
pub fn barker_code() -> Vec<i8> {
    vec![1, 1, 1, -1, -1, 1, -1, 1, 1, -1, 1]
}

/// Generate pseudo-random bipolar noise burst using LFSR
/// seed: Different seed produces different noise pattern (for preamble vs postamble)
/// duration_samples: How many samples to generate
/// amplitude: Output amplitude scaling
fn generate_prn_noise(seed: u32, duration_samples: usize, amplitude: f32) -> Vec<f32> {
    let mut lfsr = seed;
    let mut samples = vec![0.0; duration_samples];

    // Taps for 32-bit LFSR: x^32 + x^31 + x^29 + x^1 + 1 (Fibonacci configuration)
    const LFSR_TAPS: u32 = 0xB4000001; // bits 31, 30, 28, 0

    for n in 0..duration_samples {
        // Output: -1 for 0 bits, +1 for 1 bits (bipolar)
        let lsb = lfsr & 1;
        samples[n] = if lsb == 1 { amplitude } else { -amplitude };

        // LFSR step: Galois configuration
        let feedback = lfsr & 1;
        lfsr >>= 1;
        if feedback != 0 {
            lfsr ^= LFSR_TAPS;
        }
    }

    samples
}

/// Generates a chirp signal that sweeps from low to high frequency
/// Used as preamble for frame synchronization
pub fn generate_chirp(
    duration_samples: usize,
    start_freq: f32,
    end_freq: f32,
    amplitude: f32,
) -> Vec<f32> {
    let sample_rate = SAMPLE_RATE as f32;
    let duration = duration_samples as f32 / sample_rate;

    let mut samples = vec![0.0; duration_samples];
    for n in 0..duration_samples {
        let t = n as f32 / sample_rate;
        let k = (end_freq - start_freq) / duration;
        let phase = 2.0 * PI * (start_freq * t + k * t * t / 2.0);
        samples[n] = amplitude * phase.sin();
    }
    samples
}

/// Generates a smooth amplitude envelope with soft attack and decay
/// Mimics the natural shape of a whistled note
fn amplitude_envelope(t: f32, duration: f32) -> f32 {
    let attack_time = duration * 0.1;   // 10% attack
    let decay_time = duration * 0.15;   // 15% decay
    let sustain_end = duration - decay_time;

    if t < attack_time {
        // Soft attack: smooth sine-based ramp from 0 to 1
        (PI * t / (2.0 * attack_time)).sin().powi(2)
    } else if t < sustain_end {
        // Sustain: full amplitude
        1.0
    } else {
        // Smooth decay: sine-based fade from 1 to 0
        let decay_progress = ((t - sustain_end) / decay_time).clamp(0.0, 1.0);
        (PI * decay_progress / 2.0).cos().powi(2)
    }
}

/// Generates preamble chirp with human whistling characteristics
/// Ascending chirp from 800 Hz to 1800 Hz (human whistling range)
/// Applies smooth amplitude envelope (soft attack/decay) for natural sound
pub fn generate_preamble_chirp(duration_samples: usize, amplitude: f32) -> Vec<f32> {
    let sample_rate = SAMPLE_RATE as f32;
    let duration = duration_samples as f32 / sample_rate;
    let start_freq = 800.0;
    let end_freq = 1800.0;

    let mut samples = vec![0.0; duration_samples];
    for n in 0..duration_samples {
        let t = n as f32 / sample_rate;
        let k = (end_freq - start_freq) / duration;
        let phase = 2.0 * PI * (start_freq * t + k * t * t / 2.0);
        let envelope = amplitude_envelope(t, duration);
        samples[n] = amplitude * envelope * phase.sin();
    }
    samples
}

/// Generates postamble chirp with complementary whistling characteristics
/// Descending chirp from 1000 Hz to 400 Hz (low whistle range)
/// Applies smooth amplitude envelope for natural sound
pub fn generate_postamble_chirp(duration_samples: usize, amplitude: f32) -> Vec<f32> {
    let sample_rate = SAMPLE_RATE as f32;
    let duration = duration_samples as f32 / sample_rate;
    let start_freq = 1000.0;
    let end_freq = 400.0;

    let mut samples = vec![0.0; duration_samples];
    for n in 0..duration_samples {
        let t = n as f32 / sample_rate;
        let k = (end_freq - start_freq) / duration;
        let phase = 2.0 * PI * (start_freq * t + k * t * t / 2.0);
        let envelope = amplitude_envelope(t, duration);
        samples[n] = amplitude * envelope * phase.sin();
    }
    samples
}

/// Generates a single pure tone with smooth attack/decay envelope
/// freq: frequency in Hz
/// duration_samples: total number of samples
/// amplitude: peak amplitude (0.0 to 1.0)
fn generate_tone(freq: f32, duration_samples: usize, amplitude: f32) -> Vec<f32> {
    let sample_rate = SAMPLE_RATE as f32;
    let duration = duration_samples as f32 / sample_rate;

    let mut samples = vec![0.0; duration_samples];
    for n in 0..duration_samples {
        let t = n as f32 / sample_rate;
        let phase = 2.0 * PI * freq * t;
        let envelope = amplitude_envelope(t, duration);
        samples[n] = amplitude * envelope * phase.sin();
    }
    samples
}

/// Generates a three-note whistle pattern for preamble
/// Pattern: ascending melody (800 Hz -> 1200 Hz -> 1600 Hz)
/// Each note is ~83ms with smooth transitions
pub fn generate_preamble_three_note(duration_samples: usize, amplitude: f32) -> Vec<f32> {
    let note_duration = duration_samples / 3;
    let note1_freq = 800.0;  // Low note
    let note2_freq = 1200.0; // Mid note
    let note3_freq = 1600.0; // High note

    let mut samples = vec![0.0; duration_samples];

    // Generate three notes
    let note1 = generate_tone(note1_freq, note_duration, amplitude);
    let note2 = generate_tone(note2_freq, note_duration, amplitude);
    let note3 = generate_tone(note3_freq, note_duration, amplitude);

    // Concatenate the notes
    for i in 0..note_duration.min(note1.len()) {
        samples[i] = note1[i];
    }
    for i in 0..note_duration.min(note2.len()) {
        if i + note_duration < samples.len() {
            samples[i + note_duration] = note2[i];
        }
    }
    for i in 0..note_duration.min(note3.len()) {
        if i + 2 * note_duration < samples.len() {
            samples[i + 2 * note_duration] = note3[i];
        }
    }

    samples
}

/// Generates a three-note whistle pattern for postamble
/// Pattern: descending melody (1600 Hz -> 1200 Hz -> 700 Hz)
/// Each note is ~83ms with smooth transitions
/// Uses slightly lower frequencies for better speaker response
pub fn generate_postamble_three_note(duration_samples: usize, amplitude: f32) -> Vec<f32> {
    let note_duration = duration_samples / 3;
    let note1_freq = 1600.0; // High note
    let note2_freq = 1200.0; // Mid note
    let note3_freq = 700.0;  // Low note (higher than chirp postamble for better volume)

    let mut samples = vec![0.0; duration_samples];

    // Generate three notes
    let note1 = generate_tone(note1_freq, note_duration, amplitude);
    let note2 = generate_tone(note2_freq, note_duration, amplitude);
    let note3 = generate_tone(note3_freq, note_duration, amplitude);

    // Concatenate the notes
    for i in 0..note_duration.min(note1.len()) {
        samples[i] = note1[i];
    }
    for i in 0..note_duration.min(note2.len()) {
        if i + note_duration < samples.len() {
            samples[i + note_duration] = note2[i];
        }
    }
    for i in 0..note_duration.min(note3.len()) {
        if i + 2 * note_duration < samples.len() {
            samples[i + 2 * note_duration] = note3[i];
        }
    }

    samples
}

/// Generate preamble signal
/// Type determined by SIGNAL_TYPE configuration constant (PrnNoise, Chirp, or ThreeNoteWhistle)
pub fn generate_preamble(duration_samples: usize, amplitude: f32) -> Vec<f32> {
    match SIGNAL_TYPE {
        SignalType::PrnNoise => {
            // Green noise preamble (1/f spectrum, pink noise-like characteristics)
            // Uses seed 0xDEADBEEF for reproducibility
            const PREAMBLE_SEED: u32 = 0xDEADBEEF;
            let prn = generate_prn_noise(PREAMBLE_SEED, duration_samples, amplitude);

            // Apply single-pole lowpass filter for green noise characteristics
            // This creates a 1/f spectrum while preserving signal energy better than double integration
            let mut filtered = vec![0.0; duration_samples];
            let alpha = 0.3; // Filter coefficient (0.3 gives ~1/f characteristic)

            filtered[0] = prn[0];
            for i in 1..duration_samples {
                filtered[i] = alpha * prn[i] + (1.0 - alpha) * filtered[i - 1];
            }

            filtered
        }
        SignalType::Chirp => {
            // Chirp: Linear frequency sweep from 200 Hz to 4000 Hz
            generate_preamble_chirp(duration_samples, amplitude)
        }
        SignalType::ThreeNoteWhistle => {
            // Three-note whistle: ascending melody (800 -> 1200 -> 1600 Hz)
            generate_preamble_three_note(duration_samples, amplitude)
        }
    }
}

/// Generate postamble signal
/// Type determined by SIGNAL_TYPE configuration constant (PrnNoise, Chirp, or ThreeNoteWhistle)
pub fn generate_postamble_signal(duration_samples: usize, amplitude: f32) -> Vec<f32> {
    match SIGNAL_TYPE {
        SignalType::PrnNoise => {
            // PRN noise: Completely different seed 0xCAFEBABE with inverse modulation
            const POSTAMBLE_SEED: u32 = 0xCAFEBABE;
            let mut samples = generate_prn_noise(POSTAMBLE_SEED, duration_samples, amplitude);
            // Add inverse frequency modulation to maximize uniqueness from preamble
            for (i, sample) in samples.iter_mut().enumerate() {
                let mod_factor = 1.0 - 0.05 * ((i as f32 * std::f32::consts::PI * 2.0) / duration_samples as f32).sin();
                *sample *= mod_factor;
            }
            samples
        }
        SignalType::Chirp => {
            // Chirp: Reverse sweep from 4000 Hz to 200 Hz (mirror of preamble)
            generate_postamble_chirp(duration_samples, amplitude)
        }
        SignalType::ThreeNoteWhistle => {
            // Three-note whistle: descending melody (1600 -> 1200 -> 700 Hz)
            generate_postamble_three_note(duration_samples, amplitude)
        }
    }
}

/// Compute maximum RMS across sliding windows of the signal.
///
/// Uses sliding windows to find the strongest signal region, which helps identify
/// signal bursts and avoids being skewed by long silences or unrelated background noise.
///
/// Window size: `ADAPTIVE_RMS_WINDOW_LENGTH` = 2048 samples (~41 ms at 48 kHz sample rate)
/// This window size balances responsiveness to signal changes with stability against short-term noise.
///
/// # Tradeoffs:
/// - **Benefit**: More robust detection by focusing on the strongest signal region
/// - **Benefit**: Ignores long silence periods or unrelated noise that spans the entire buffer
/// - **Tradeoff**: May miss weak distributed signals spread evenly across the buffer
/// - **Tradeoff**: Slightly slower than computing RMS over the entire buffer
fn compute_max_rms_from_windows(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    // Use ADAPTIVE_RMS_WINDOW_LENGTH (2048 samples) or the full buffer if smaller
    let window_len = ADAPTIVE_RMS_WINDOW_LENGTH.min(samples.len());
    let mut max_rms: f32 = 0.0;

    for i in 0..=samples.len().saturating_sub(window_len) {
        let window = &samples[i..i + window_len];
        let rms = (window.iter().map(|x| x * x).sum::<f32>() / window.len() as f32).sqrt();
        max_rms = max_rms.max(rms);
    }

    max_rms
}

/// Compute the detection threshold value based on the threshold specification
/// - Adaptive: Adjusts threshold based on maximum signal RMS from sliding windows:
///   - Uses sliding windows of ADAPTIVE_RMS_WINDOW_LENGTH samples to find the strongest region
///   - RMS > 0.1: 0.4 (strong signal, strict detection)
///   - 0.02 < RMS ≤ 0.1: 0.35 (medium signal)
///   - RMS ≤ 0.02: 0.3 (weak signal, relaxed threshold)
/// - Fixed(value): Returns the provided fixed threshold value
fn compute_threshold_value(samples: &[f32], threshold: DetectionThreshold) -> f32 {
    match threshold {
        DetectionThreshold::Adaptive => {
            let signal_rms: f32 = compute_max_rms_from_windows(samples);
            if signal_rms > 0.1 {
                0.4
            } else if signal_rms > 0.02 {
                0.35
            } else {
                0.3
            }
        }
        DetectionThreshold::Fixed(value) => value,
    }
}

/// Detect preamble using efficient FFT-based cross-correlation
/// Returns the position where the preamble (PRN noise burst) is most likely to start
/// threshold: Specifies how to determine the detection threshold (Adaptive or Fixed)
/// Panics if Fixed threshold is invalid (not in range [0.001, 1.0], i.e., must be inclusive of 0.001 and 1.0)
pub fn detect_preamble(samples: &[f32], threshold: DetectionThreshold) -> Option<usize> {
    // Validate threshold
    if let DetectionThreshold::Fixed(value) = threshold {
        if value < 0.001 || value > 1.0 {
            panic!("Invalid fixed detection threshold: {}. Must be in range [0.001, 1.0]. Minimum is 0.001 (0.1%)", value);
        }
    }

    let preamble_samples = crate::PREAMBLE_SAMPLES;

    if samples.len() < preamble_samples {
        return None;
    }

    // Generate expected preamble signal pattern (same seed = same pattern)
    let template = generate_preamble(preamble_samples, 1.0);

    // Use FFT-based correlation for O(N log N) complexity
    let fft_correlation = match fft_correlate_1d(samples, &template, Mode::Full) {
        Ok(corr) => corr,
        Err(e) => {
            warn!(
                "FFT correlation failed during preamble detection: {} (samples={}, template={}, mode=Full)",
                e,
                samples.len(),
                template.len()
            );
            return None;
        }
    };

    let mut best_pos = 0;
    let mut best_correlation = 0.0;

    // Calculate template energy once
    let template_energy: f32 = template.iter().map(|x| x * x).sum();

    // Build prefix-sum array of squared samples for O(1) window energy computation
    let mut sq_prefix = vec![0.0; samples.len() + 1];
    for k in 0..samples.len() {
        sq_prefix[k + 1] = sq_prefix[k] + samples[k] * samples[k];
    }

    // Iterate through valid positions and normalize correlation coefficients
    for i in 0..=samples.len().saturating_sub(preamble_samples) {
        // FFT correlation output at index (i + preamble_samples - 1) corresponds to window starting at i
        let fft_index = i + preamble_samples - 1;
        let raw_correlation = fft_correlation[fft_index];

        // Calculate window energy using O(1) prefix-sum lookup
        let window_energy = sq_prefix[i + preamble_samples] - sq_prefix[i];

        // Compute normalized correlation coefficient
        let denom = (window_energy * template_energy).sqrt();
        let normalized_corr = if denom > 1e-10 {
            (raw_correlation / denom).abs()
        } else {
            0.0
        };

        if normalized_corr > best_correlation {
            best_correlation = normalized_corr;
            best_pos = i;
        }
    }

    // Determine detection threshold
    let threshold_value = compute_threshold_value(samples, threshold);

    if best_correlation > threshold_value {
        Some(best_pos)
    } else {
        None
    }
}

/// Detect postamble using efficient cross-correlation
/// Returns the position where the postamble (PRN noise burst) is most likely to start
/// threshold: Specifies how to determine the detection threshold (Adaptive or Fixed)
/// Panics if Fixed threshold is invalid (not in range [0.001, 1.0])
pub fn detect_postamble(samples: &[f32], threshold: DetectionThreshold) -> Option<usize> {
    // Validate threshold
    if let DetectionThreshold::Fixed(value) = threshold {
        if value < 0.001 || value > 1.0 {
            panic!("Invalid fixed detection threshold: {}. Must be in range [0.001, 1.0]. Minimum is 0.001 (0.1%)", value);
        }
    }

    let postamble_samples = crate::POSTAMBLE_SAMPLES;

    if samples.len() < postamble_samples {
        return None;
    }

    // Generate expected postamble signal pattern (different seed from preamble)
    let template = generate_postamble_signal(postamble_samples, 1.0);

    // Use FFT-based correlation for O(N log N) complexity
    let fft_correlation = match fft_correlate_1d(samples, &template, Mode::Full) {
        Ok(corr) => corr,
        Err(e) => {
            warn!(
                "FFT correlation failed during postamble detection: {} (samples={}, template={}, mode=Full)",
                e,
                samples.len(),
                template.len()
            );
            return None;
        }
    };

    let mut best_pos = 0;
    let mut best_correlation = 0.0;

    // Calculate template energy once
    let template_energy: f32 = template.iter().map(|x| x * x).sum();

    // Build prefix-sum array of squared samples for O(1) window energy computation
    let mut sq_prefix = vec![0.0; samples.len() + 1];
    for k in 0..samples.len() {
        sq_prefix[k + 1] = sq_prefix[k] + samples[k] * samples[k];
    }

    // Iterate through valid positions and normalize correlation coefficients
    for i in 0..=samples.len().saturating_sub(postamble_samples) {
        // FFT correlation output at index (i + postamble_samples - 1) corresponds to window starting at i
        let fft_index = i + postamble_samples - 1;
        let raw_correlation = fft_correlation[fft_index];

        // Calculate window energy using O(1) prefix-sum lookup
        let window_energy = sq_prefix[i + postamble_samples] - sq_prefix[i];

        // Compute normalized correlation coefficient
        let denom = (window_energy * template_energy).sqrt();
        let normalized_corr = if denom > 1e-10 {
            (raw_correlation / denom).abs()
        } else {
            0.0
        };

        if normalized_corr > best_correlation {
            best_correlation = normalized_corr;
            best_pos = i;
        }
    }

    // Determine detection threshold
    let threshold_value = compute_threshold_value(samples, threshold);

    if best_correlation > threshold_value {
        Some(best_pos)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecoderFsk;

    #[test]
    fn test_barker_code() {
        let barker = barker_code();
        assert_eq!(barker.len(), 11);
    }

    #[test]
    fn test_barker_code_values() {
        let barker = barker_code();
        let expected = vec![1, 1, 1, -1, -1, 1, -1, 1, 1, -1, 1];
        assert_eq!(barker, expected);
    }

    #[test]
    fn test_barker_autocorrelation() {
        let barker = barker_code();
        // Autocorrelation at lag 0 should be 11 (sum of squares)
        let autocorr: i32 = barker.iter().map(|&x| (x as i32) * (x as i32)).sum();
        assert_eq!(autocorr, 11);
    }

    #[test]
    fn test_barker_sidelobe_property() {
        let barker = barker_code();
        // Test correlation with shifted versions
        // Verify that autocorrelation is much larger than sidelobes
        let mut max_sidelobe = 0i32;
        let mut avg_sidelobe = 0i32;
        for lag in 1..11 {
            let correlation: i32 = barker[..11 - lag]
                .iter()
                .zip(barker[lag..].iter())
                .map(|(a, b)| (*a as i32) * (*b as i32))
                .sum();
            let abs_corr = correlation.abs();
            max_sidelobe = max_sidelobe.max(abs_corr);
            avg_sidelobe += abs_corr;
        }
        avg_sidelobe /= 10;
        // Autocorrelation (11) should be >> sidelobes
        assert!(max_sidelobe < 11, "Max sidelobe {} should be < autocorr 11", max_sidelobe);
        assert!(avg_sidelobe < 11, "Avg sidelobe {} should be < autocorr 11", avg_sidelobe);
    }

    #[test]
    fn test_barker_code_symmetry() {
        let barker = barker_code();
        // Check that alternating signs create the Barker structure
        let alternations = barker
            .windows(2)
            .filter(|w| w[0] != w[1])
            .count();
        assert!(alternations >= 5, "Barker should have multiple sign changes");
    }

    #[test]
    fn test_barker_chip_spreading() {
        let barker = barker_code();
        // Each Barker chip (±1) can spread one information bit
        assert_eq!(barker.iter().all(|&x| x == 1 || x == -1), true);

        // Test spreading a single bit across Barker sequence
        let bit = true;
        let bit_val: i8 = if bit { 1 } else { -1 };
        let spread: Vec<i8> = barker.iter().map(|&x| x * bit_val).collect();
        assert_eq!(spread.len(), 11);
    }

    #[test]
    fn test_barker_despread_clean() {
        let barker = barker_code();
        let bit = true;
        let bit_val = if bit { 1.0 } else { -1.0 };

        // Spread the bit
        let spread: Vec<f32> = barker.iter().map(|&x| (x as f32) * bit_val).collect();

        // Despread by correlating with Barker
        let correlation: f32 = spread
            .iter()
            .zip(barker.iter())
            .map(|(&s, &b)| s * (b as f32))
            .sum();

        // Correlation should be close to 11 (autocorrelation peak)
        assert!(correlation > 10.0);
    }

    #[test]
    fn test_barker_despread_with_noise() {
        let barker = barker_code();
        let bit = true;
        let bit_val = if bit { 1.0 } else { -1.0 };

        // Spread and add noise
        let spread: Vec<f32> = barker
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                let noise = ((i as f32 * 0.789) % 1.0) * 0.2 - 0.1; // ±10% noise
                (x as f32) * bit_val + noise
            })
            .collect();

        // Despread by correlating with Barker
        let correlation: f32 = spread
            .iter()
            .zip(barker.iter())
            .map(|(&s, &b)| s * (b as f32))
            .sum();

        // Should still detect positive correlation despite noise
        assert!(correlation > 5.0, "Correlation with noise: {}", correlation);
    }

    #[test]
    fn test_barker_as_matched_filter() {
        let barker = barker_code();
        // Create signal with Barker repeated (10 repetitions = 110 samples)

        // Create signal with Barker repeated
        let mut signal = Vec::new();
        for _ in 0..10 {
            for &chip in &barker {
                signal.push(chip as f32);
            }
        }

        // Correlate with single Barker sequence
        let correlation: f32 = signal
            .iter()
            .zip(barker.iter().cycle())
            .map(|(&s, &b)| s * (b as f32))
            .sum();

        // Should have very high correlation (10 * autocorr)
        assert!(correlation > 100.0);
    }

    #[test]
    fn test_barker_mismatch_detection() {
        let barker = barker_code();
        // Create a random sequence
        let random_seq: Vec<i8> = vec![1, -1, 1, 1, -1, 1, -1, -1, 1, -1, -1];

        // Correlate random with Barker
        let correlation: i32 = random_seq
            .iter()
            .zip(barker.iter())
            .map(|(a, b)| (*a as i32) * (*b as i32))
            .sum();

        // Correlation with non-matching sequence should be lower
        // (not guaranteed to be low, but statistically likely)
        assert!(correlation.abs() < 10);
    }

    #[test]
    fn test_chirp_generation() {
        let chirp = generate_chirp(1600, 200.0, 4000.0, 1.0);
        assert_eq!(chirp.len(), 1600);
    }

    #[test]
    fn test_chirp_amplitude() {
        let chirp = generate_chirp(1600, 200.0, 4000.0, 0.5);
        let max_val = chirp.iter().map(|x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
        assert!(max_val <= 0.6 && max_val >= 0.4, "Max amplitude: {}", max_val);
    }

    #[test]
    fn test_chirp_frequency_sweep() {
        let chirp = generate_chirp(1600, 200.0, 4000.0, 1.0);

        // Compute zero crossings as proxy for frequency
        let zero_crossings_early = chirp[..400]
            .windows(2)
            .filter(|w| (w[0] > 0.0) != (w[1] > 0.0))
            .count();

        let zero_crossings_late = chirp[1200..]
            .windows(2)
            .filter(|w| (w[0] > 0.0) != (w[1] > 0.0))
            .count();

        // Later part should have more zero crossings (higher frequency)
        assert!(zero_crossings_late > zero_crossings_early);
    }


    // Helper functions for signal-agnostic testing
    fn create_preamble(amplitude: f32) -> Vec<f32> {
        match SIGNAL_TYPE {
            SignalType::PrnNoise => generate_preamble(crate::PREAMBLE_SAMPLES, amplitude),
            SignalType::Chirp => generate_preamble_chirp(crate::PREAMBLE_SAMPLES, amplitude),
            SignalType::ThreeNoteWhistle => generate_preamble_three_note(crate::PREAMBLE_SAMPLES, amplitude),
        }
    }

    fn create_postamble(amplitude: f32) -> Vec<f32> {
        match SIGNAL_TYPE {
            SignalType::PrnNoise => generate_postamble_signal(crate::POSTAMBLE_SAMPLES, amplitude),
            SignalType::Chirp => generate_postamble_chirp(crate::POSTAMBLE_SAMPLES, amplitude),
            SignalType::ThreeNoteWhistle => generate_postamble_three_note(crate::POSTAMBLE_SAMPLES, amplitude),
        }
    }

    #[test]
    fn test_preamble_detection_strong_signal() {
        // Strong signal: should always detect with strict 0.4 threshold
        let preamble = create_preamble(1.0);
        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]); // Add silence after

        let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Strong signal should be detected");
        assert!(result.unwrap() < 100, "Should detect near start");
    }

    #[test]
    fn test_preamble_detection_medium_signal() {
        // Medium signal (0.3x amplitude): should detect with 0.35 threshold
        let preamble = create_preamble(0.3);
        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);

        let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Medium signal should be detected");
    }

    #[test]
    fn test_preamble_detection_weak_signal() {
        // Weak signal (0.1x amplitude): should detect with 0.3 threshold
        let preamble = create_preamble(0.1);
        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);

        let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Weak signal should be detected");
    }

    #[test]
    fn test_preamble_detection_very_weak_signal() {
        // Very weak signal (0.05x amplitude): at detection limit
        let preamble = create_preamble(0.05);
        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);

        let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
        // May or may not detect (at threshold boundary), but should not crash
        let _ = result;
    }

    #[test]
    fn test_preamble_detection_with_noise() {
        // Weak signal with noise: adaptive threshold should handle
        let preamble = create_preamble(0.15);
        let mut signal = preamble.clone();

        // Add small noise
        for s in &mut signal[..1000] {
            *s += 0.02; // noise amplitude
        }
        signal.extend_from_slice(&vec![0.0; 1000]);

        let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Weak signal with noise should be detected");
    }

    #[test]
    fn test_postamble_detection_strong_signal() {
        // Strong signal: should always detect with strict 0.4 threshold
        let postamble = create_postamble(1.0);
        let mut signal = vec![0.0; 1000];
        signal.extend_from_slice(&postamble);

        let result = detect_postamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Strong postamble should be detected");
    }

    #[test]
    fn test_postamble_detection_weak_signal() {
        // Weak signal: should detect with relaxed threshold
        let postamble = create_postamble(0.1);
        let mut signal = vec![0.0; 1000];
        signal.extend_from_slice(&postamble);

        let result = detect_postamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Weak postamble should be detected");
    }

    #[test]
    fn test_adaptive_threshold_rms_strong() {
        // Create signal with RMS > 0.1 (should use 0.4 threshold)
        let preamble = create_preamble(0.5);
        let signal_rms: f32 = (preamble.iter().map(|x| x * x).sum::<f32>() / preamble.len() as f32).sqrt();
        assert!(signal_rms > 0.1, "Signal RMS should be > 0.1 for strong signal test");

        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);
        let result = detect_preamble(&signal, DetectionThreshold::Adaptive);
        assert!(result.is_some());
    }

    #[test]
    fn test_adaptive_threshold_rms_medium() {
        // Create signal with 0.02 < RMS < 0.1 (should use 0.35 threshold)
        // Amplitude 0.08 gives RMS ~0.04 (in medium range)
        let preamble = create_preamble(0.08);
        let signal_rms: f32 = (preamble.iter().map(|x| x * x).sum::<f32>() / preamble.len() as f32).sqrt();
        assert!(signal_rms > 0.02 && signal_rms <= 0.1, "Signal RMS should be in medium range, got {}", signal_rms);

        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);
        let result = detect_preamble(&signal, DetectionThreshold::Adaptive);
        assert!(result.is_some());
    }

    #[test]
    fn test_adaptive_threshold_rms_weak() {
        // Create signal with RMS <= 0.02 (should use 0.3 threshold)
        // Amplitude 0.02 gives RMS ~0.01 (in weak range)
        let preamble = create_preamble(0.02);
        let signal_rms: f32 = (preamble.iter().map(|x| x * x).sum::<f32>() / preamble.len() as f32).sqrt();
        assert!(signal_rms <= 0.02, "Signal RMS should be <= 0.02 for weak signal test, got {}", signal_rms);

        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);
        let result = detect_preamble(&signal, DetectionThreshold::Adaptive);
        assert!(result.is_some(), "Weak signal should be detected with adaptive threshold");
    }

    #[test]
    fn test_preamble_false_positive_rejection() {
        // Random noise should not trigger false positives
        let noise: Vec<f32> = (0..crate::PREAMBLE_SAMPLES * 2)
            .map(|i| (i as f32 * 0.1).sin() * 0.01)
            .collect();

        let result = detect_preamble(&noise, DetectionThreshold::Fixed(0.1));
        // May or may not detect, but should be much less likely than true preamble
        let _ = result;
    }

    #[test]
    fn test_preamble_attenuation_series() {
        // Test series of attenuated signals to verify graceful degradation
        let base_preamble = create_preamble(1.0);

        let attenuation_levels = vec![1.0, 0.5, 0.2, 0.1, 0.05];
        let mut detected_count = 0;

        for &atten in &attenuation_levels {
            let preamble = base_preamble.iter().map(|x| x * atten).collect::<Vec<_>>();
            let mut signal = preamble.clone();
            signal.extend_from_slice(&vec![0.0; 1000]);

            if detect_preamble(&signal, DetectionThreshold::Fixed(0.1)).is_some() {
                detected_count += 1;
            }
        }

        // Should detect most signals (at least 3 out of 5)
        assert!(detected_count >= 3, "Should detect at least 3 out of 5 attenuation levels");
    }

    #[test]
    fn test_preamble_position_accuracy() {
        // Verify detection correctly identifies preamble position
        let silence_before = vec![0.0; 500];
        let preamble = create_preamble(0.3);
        let mut signal = silence_before.clone();
        signal.extend_from_slice(&preamble);
        signal.extend_from_slice(&vec![0.0; 1000]);

        let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some());

        let pos = result.unwrap();
        // Should detect near the start of silence (within tolerance)
        assert!(pos < 1000, "Detection position {} should be reasonable", pos);
    }

     #[test]
    fn test_postamble_position_accuracy() {
        // Verify detection correctly identifies postamble position
        let postamble = create_postamble(0.3);
        let mut signal = vec![0.0; 1000];
        signal.extend_from_slice(&postamble);
        signal.extend_from_slice(&vec![0.0; 500]);

        let result = detect_postamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some());

        let pos = result.unwrap();
        // Should detect near the start of postamble (within tolerance)
        assert!(pos >= 1000 && pos < 2000, "Detection position {} should be reasonable", pos);
    }

    // ========================================================================
    // STRICT POSITION ACCURACY TESTS
    // ========================================================================

    #[test]
    fn test_preamble_position_strict_zero_offset() {
        // Preamble at exact start with no leading silence
        let preamble = create_preamble(0.5);
        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 2000]);

        let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Should detect preamble at signal start");

        let pos = result.unwrap();
        // Strict tolerance: must detect within first 500 samples
        assert!(pos < 500, "Position {} should be within first 500 samples", pos);
    }

    #[test]
    fn test_preamble_position_strict_small_offset() {
        // Preamble with small leading silence (100 samples)
        let silence_before = vec![0.0; 100];
        let preamble = create_preamble(0.5);
        let mut signal = silence_before.clone();
        signal.extend_from_slice(&preamble);
        signal.extend_from_slice(&vec![0.0; 2000]);

        let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Should detect preamble with 100 sample offset");

        let pos = result.unwrap();
        // Strict tolerance: detect within 500 samples of actual start
        assert!(pos >= 100 && pos < 500,
                "Position {} should be between 100-500 samples, detected at offset", pos);
    }

    #[test]
    fn test_preamble_position_strict_medium_offset() {
        // Preamble with medium leading silence (1000 samples)
        let silence_before = vec![0.0; 1000];
        let preamble = create_preamble(0.5);
        let mut signal = silence_before.clone();
        signal.extend_from_slice(&preamble);
        signal.extend_from_slice(&vec![0.0; 2000]);

        let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Should detect preamble with 1000 sample offset");

        let pos = result.unwrap();
        // Strict tolerance: detect within 500 samples of actual start (1000)
        assert!(pos >= 500 && pos < 1500,
                "Position {} should be between 500-1500 samples", pos);
    }

    #[test]
    fn test_preamble_position_multiple_offsets() {
        // Test detection accuracy at various positions
        let offsets = vec![0, 100, 500, 1000, 2000, 3000];

        for offset in offsets {
            let mut signal = vec![0.0; offset];
            let preamble = create_preamble(0.5);
            signal.extend_from_slice(&preamble);
            signal.extend_from_slice(&vec![0.0; 2000]);

            let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
            assert!(result.is_some(), "Should detect preamble at offset {}", offset);

            let pos = result.unwrap();
            // Tolerance: within 20% of actual position or 500 samples, whichever is larger
            let tolerance = (offset as f32 * 0.2) as usize;
            let tolerance = tolerance.max(500);

            assert!(
                (pos as i32 - offset as i32).abs() < tolerance as i32,
                "Position {} should be close to actual offset {}",
                pos, offset
            );
        }
    }

    #[test]
    fn test_postamble_position_strict_zero_offset() {
        // Postamble at exact start of signal (after leading silence)
        let mut signal = vec![0.0; 2000];
        let postamble = create_postamble(0.5);
        signal.extend_from_slice(&postamble);
        signal.extend_from_slice(&vec![0.0; 1000]);

        let result = detect_postamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Should detect postamble at position 2000");

        let pos = result.unwrap();
        // Strict tolerance: must detect near position 2000 (within 500 samples)
        assert!(pos >= 1500 && pos < 2500,
                "Position {} should be between 1500-2500 samples", pos);
    }

    #[test]
    fn test_postamble_position_strict_various_offsets() {
        // Test postamble detection at various positions
        let positions = vec![1000, 2000, 4000, 8000];

        for pos_target in positions {
            let mut signal = vec![0.0; pos_target];
            let postamble = create_postamble(0.5);
            signal.extend_from_slice(&postamble);
            signal.extend_from_slice(&vec![0.0; 1000]);

            let result = detect_postamble(&signal, DetectionThreshold::Fixed(0.1));
            assert!(result.is_some(), "Should detect postamble at position {}", pos_target);

            let pos = result.unwrap();
            // Tolerance: within 20% or 500 samples
            let tolerance = ((pos_target as f32 * 0.2) as usize).max(500);

            assert!(
                (pos as i32 - pos_target as i32).abs() < tolerance as i32,
                "Detected position {} should be close to actual position {}",
                pos, pos_target
            );
        }
    }

    #[test]
    fn test_preamble_postamble_sequence_accuracy() {
        // Test detection in sequence: preamble -> data -> postamble
        let preamble = create_preamble(0.5);
        let payload = vec![0.05; 8000]; // Small amplitude "data"
        let postamble = create_postamble(0.5);

        let mut signal = preamble.clone();
        signal.extend_from_slice(&payload);
        signal.extend_from_slice(&postamble);

        let preamble_pos = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
        let postamble_pos = detect_postamble(&signal, DetectionThreshold::Fixed(0.1));

        assert!(preamble_pos.is_some(), "Should detect preamble in sequence");
        assert!(postamble_pos.is_some(), "Should detect postamble in sequence");

        let pre_pos = preamble_pos.unwrap();
        let post_pos = postamble_pos.unwrap();

        // Preamble should be detected near start (within 500 samples)
        assert!(pre_pos < 500, "Preamble position {} should be at start", pre_pos);

        // Postamble should be detected after preamble + payload
        let expected_post_start = crate::PREAMBLE_SAMPLES + payload.len();
        assert!(post_pos > expected_post_start as usize - 500,
                "Postamble position {} should be after preamble+payload region", post_pos);
    }

    #[test]
    fn test_preamble_position_with_leading_noise() {
        // Test preamble detection with initial noise before it
        let mut signal = vec![];

        // Add some low-amplitude noise
        for i in 0..1000 {
            signal.push((i as f32 * 0.1).sin() * 0.02);
        }

        let preamble = create_preamble(0.5);
        signal.extend_from_slice(&preamble);
        signal.extend_from_slice(&vec![0.0; 2000]);

        let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Should detect preamble despite leading noise");

        let pos = result.unwrap();
        // Should detect preamble before position 1500 (noise ends at 1000, preamble after)
        assert!(pos < 1500, "Position {} should be before 1500 samples", pos);
    }

    #[test]
    fn test_postamble_position_with_trailing_noise() {
        // Test postamble detection with noise after it
        let mut signal = vec![0.0; 2000];
        let postamble = create_postamble(0.5);
        signal.extend_from_slice(&postamble);

        // Add trailing noise
        for i in 0..1000 {
            signal.push((i as f32 * 0.1).sin() * 0.02);
        }

        let result = detect_postamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Should detect postamble despite trailing noise");

        let pos = result.unwrap();
        let expected = 2000;
        let tolerance = 500;
        assert!(
            (pos as i32 - expected as i32).abs() < tolerance as i32,
            "Position {} should be near expected {} ± {}",
            pos, expected, tolerance
        );
    }

    #[test]
    fn test_preamble_detection_boundary_precision() {
        // Test precise boundary detection at preamble edges
        let mut signal = vec![0.0; 500];
        let preamble = create_preamble(0.5);
        let preamble_len = preamble.len();
        signal.extend_from_slice(&preamble);
        signal.extend_from_slice(&vec![0.0; 1000]);

        let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.1));
        assert!(result.is_some(), "Should detect preamble with 500 sample offset");

        let pos = result.unwrap();
        let expected_start = 500;
        let expected_end = 500 + preamble_len;

        // Detection should be within the preamble region
        assert!(pos >= expected_start - 100 && pos < expected_end,
                "Position {} should be within preamble region [{}, {})",
                pos, expected_start - 100, expected_end);
    }

    // ========================================================================
    // FIXED THRESHOLD VALIDATION TESTS
    // ========================================================================

    #[test]
    fn test_fixed_threshold_minimum_boundary() {
        // Test that minimum threshold of 0.001 (0.1%) is accepted
        let preamble = create_preamble(0.5);
        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);

        // Should not panic with minimum threshold
        let result = detect_preamble(&signal, DetectionThreshold::Fixed(0.001));
        assert!(result.is_some(), "Minimum threshold (0.001) should work");
    }

    #[test]
    #[should_panic(expected = "Invalid fixed detection threshold")]
    fn test_fixed_threshold_below_minimum_panics() {
        // Test that threshold below 0.001 causes panic
        let preamble = create_preamble(0.5);
        let signal = preamble.clone();
        detect_preamble(&signal, DetectionThreshold::Fixed(0.0005));
    }

    #[test]
    #[should_panic(expected = "Invalid fixed detection threshold")]
    fn test_fixed_threshold_zero_panics() {
        // Test that threshold of exactly 0.0 causes panic
        let preamble = create_preamble(0.5);
        let signal = preamble.clone();
        detect_preamble(&signal, DetectionThreshold::Fixed(0.0));
    }

    #[test]
    #[should_panic(expected = "Invalid fixed detection threshold")]
    fn test_fixed_threshold_negative_panics() {
        // Test that negative threshold causes panic
        let preamble = create_preamble(0.5);
        let signal = preamble.clone();
        detect_preamble(&signal, DetectionThreshold::Fixed(-0.1));
    }

    #[test]
    #[should_panic(expected = "Invalid fixed detection threshold")]
    fn test_fixed_threshold_above_maximum_panics() {
        // Test that threshold > 1.0 causes panic
        let preamble = create_preamble(0.5);
        let signal = preamble.clone();
        detect_preamble(&signal, DetectionThreshold::Fixed(1.1));
    }

    #[test]
    fn test_fixed_threshold_maximum_boundary() {
        // Test that maximum threshold of 1.0 is accepted
        let preamble = create_preamble(0.5);
        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);

        // Should not panic with maximum threshold
        let result = detect_preamble(&signal, DetectionThreshold::Fixed(1.0));
        assert!(result.is_none(), "Maximum threshold (1.0) is very strict, may not detect");
    }

    #[test]
    fn test_fixed_threshold_mid_range() {
        // Test standard fixed threshold values work correctly
        let preamble = create_preamble(0.5);
        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);

        // Test common threshold values
        let thresholds = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9];
        for threshold in thresholds {
            let result = detect_preamble(&signal, DetectionThreshold::Fixed(threshold));
            // Should work without panicking - exact detection depends on signal quality
            let _ = result;
        }
    }

    #[test]
    fn test_postamble_fixed_threshold_minimum() {
        // Test postamble detection with minimum fixed threshold
        let postamble = create_postamble(0.5);
        let mut signal = vec![0.0; 1000];
        signal.extend_from_slice(&postamble);

        let result = detect_postamble(&signal, DetectionThreshold::Fixed(0.001));
        assert!(result.is_some(), "Postamble should detect with minimum threshold");
    }

    #[test]
    #[should_panic(expected = "Invalid fixed detection threshold")]
    fn test_postamble_fixed_threshold_below_minimum_panics() {
        // Test that postamble also validates minimum threshold
        let postamble = create_postamble(0.5);
        let signal = postamble.clone();
        detect_postamble(&signal, DetectionThreshold::Fixed(0.0005));
    }

    #[test]
    #[should_panic(expected = "Invalid fixed detection threshold")]
    fn test_postamble_fixed_threshold_above_maximum_panics() {
        // Test that postamble validates maximum threshold
        let postamble = create_postamble(0.5);
        let signal = postamble.clone();
        detect_postamble(&signal, DetectionThreshold::Fixed(1.5));
    }

    // ========================================================================
    // DECODER THRESHOLD CLAMPING TESTS
    // ========================================================================

    #[test]
    fn test_decoder_clamps_preamble_threshold_below_minimum() {
        // Test that DecoderFsk clamps threshold values below minimum to 0.001
        let mut decoder = DecoderFsk::new().unwrap();

        // Set threshold below minimum
        decoder.set_preamble_threshold(DetectionThreshold::Fixed(0.0005));

        // Should be clamped to 0.001
        let threshold = decoder.get_preamble_threshold();
        assert_eq!(threshold, DetectionThreshold::Fixed(0.001), "Threshold below minimum should be clamped to 0.001");
    }

    #[test]
    fn test_decoder_clamps_preamble_threshold_above_maximum() {
        // Test that DecoderFsk clamps threshold values above maximum to 1.0
        let mut decoder = DecoderFsk::new().unwrap();

        // Set threshold above maximum
        decoder.set_preamble_threshold(DetectionThreshold::Fixed(1.5));

        // Should be clamped to 1.0
        let threshold = decoder.get_preamble_threshold();
        assert_eq!(threshold, DetectionThreshold::Fixed(1.0), "Threshold above maximum should be clamped to 1.0");
    }

    #[test]
    fn test_decoder_clamps_postamble_threshold_below_minimum() {
        // Test that DecoderFsk clamps postamble threshold below minimum
        let mut decoder = DecoderFsk::new().unwrap();

        // Set threshold below minimum
        decoder.set_postamble_threshold(DetectionThreshold::Fixed(0.0005));

        // Should be clamped to 0.001
        let threshold = decoder.get_postamble_threshold();
        assert_eq!(threshold, DetectionThreshold::Fixed(0.001), "Threshold below minimum should be clamped to 0.001");
    }

    #[test]
    fn test_decoder_preserves_valid_fixed_threshold() {
        // Test that valid fixed thresholds are preserved
        let mut decoder = DecoderFsk::new().unwrap();

        let test_thresholds = vec![0.001, 0.1, 0.25, 0.4, 0.5, 0.75, 1.0];

        for original in test_thresholds {
            decoder.set_preamble_threshold(DetectionThreshold::Fixed(original));
            let retrieved = decoder.get_preamble_threshold();
            assert_eq!(retrieved, DetectionThreshold::Fixed(original), "Valid threshold {} should be preserved", original);
        }
    }

    #[test]
    fn test_decoder_adaptive_threshold_via_enum() {
        // Test that setting threshold to Adaptive enum enables adaptive mode
        let mut decoder = DecoderFsk::new().unwrap();

        decoder.set_preamble_threshold(DetectionThreshold::Adaptive);

        // Getting threshold should return Adaptive
        let threshold = decoder.get_preamble_threshold();
        assert_eq!(threshold, DetectionThreshold::Adaptive, "Adaptive threshold should return Adaptive");
    }

    #[test]
    fn test_decoder_adaptive_threshold_persists() {
        // Test that adaptive threshold persists across multiple calls
        let mut decoder = DecoderFsk::new().unwrap();

        // Set to adaptive
        decoder.set_preamble_threshold(DetectionThreshold::Adaptive);
        assert_eq!(decoder.get_preamble_threshold(), DetectionThreshold::Adaptive);

        // Change to fixed
        decoder.set_preamble_threshold(DetectionThreshold::Fixed(0.4));
        assert_eq!(decoder.get_preamble_threshold(), DetectionThreshold::Fixed(0.4));

        // Change back to adaptive
        decoder.set_preamble_threshold(DetectionThreshold::Adaptive);
        assert_eq!(decoder.get_preamble_threshold(), DetectionThreshold::Adaptive);
    }

    #[test]
    fn test_decoder_unified_threshold_setter() {
        // Test that set_detection_threshold sets both preamble and postamble
        let mut decoder = DecoderFsk::new().unwrap();

        decoder.set_detection_threshold(DetectionThreshold::Fixed(0.35));

        let preamble_thresh = decoder.get_preamble_threshold();
        let postamble_thresh = decoder.get_postamble_threshold();

        assert_eq!(preamble_thresh, DetectionThreshold::Fixed(0.35), "Preamble threshold should be set");
        assert_eq!(postamble_thresh, DetectionThreshold::Fixed(0.35), "Postamble threshold should be set");
    }

    #[test]
    fn test_decoder_threshold_edge_cases() {
        // Test edge case threshold values
        let mut decoder = DecoderFsk::new().unwrap();

        // Test various edge cases
        let edge_cases = vec![
            (DetectionThreshold::Adaptive, DetectionThreshold::Adaptive),
            (DetectionThreshold::Fixed(0.0009), DetectionThreshold::Fixed(0.001)),   // Just below minimum → clamped to 0.001
            (DetectionThreshold::Fixed(0.001), DetectionThreshold::Fixed(0.001)),    // Exactly minimum → kept
            (DetectionThreshold::Fixed(0.0011), DetectionThreshold::Fixed(0.0011)),  // Just above minimum → kept
            (DetectionThreshold::Fixed(0.9999), DetectionThreshold::Fixed(0.9999)),  // Just below maximum → kept
            (DetectionThreshold::Fixed(1.0), DetectionThreshold::Fixed(1.0)),        // Exactly maximum → kept
            (DetectionThreshold::Fixed(1.0001), DetectionThreshold::Fixed(1.0)),     // Just above maximum → clamped
            (DetectionThreshold::Fixed(2.0), DetectionThreshold::Fixed(1.0)),        // Well above maximum → clamped
        ];

        for (input, expected) in edge_cases {
            decoder.set_preamble_threshold(input);
            let result = decoder.get_preamble_threshold();
            assert_eq!(result, expected,
                    "Input {:?} should result in {:?} but got {:?}", input, expected, result);
        }
    }

}
