/// Audio resampling utility for converting between different sample rates
/// Uses linear interpolation for quality audio resampling

/// Mix stereo audio to mono by averaging both channels
///
/// # Arguments
/// * `samples` - Interleaved stereo audio samples [L, R, L, R, ...]
///
/// # Returns
/// Mono audio samples (averaged from both channels)
///
/// # Panics
/// If samples length is not even
pub fn stereo_to_mono(samples: &[f32]) -> Vec<f32> {
    assert!(
        samples.len() % 2 == 0,
        "Stereo audio must have even number of samples"
    );

    let mut mono = Vec::with_capacity(samples.len() / 2);
    for chunk in samples.chunks(2) {
        let averaged = (chunk[0] + chunk[1]) / 2.0;
        mono.push(averaged);
    }
    mono
}

/// Resample audio to a target sample rate using linear interpolation
///
/// # Arguments
/// * `samples` - Input audio samples
/// * `from_rate` - Current sample rate in Hz
/// * `to_rate` - Target sample rate in Hz
///
/// # Returns
/// Resampled audio at the target sample rate
///
/// # Example
/// ```ignore
/// let audio_48k = vec![0.1, 0.2, 0.3, ...]; // audio at 48kHz
/// let audio_16k = resample_audio(&audio_48k, 48000, 16000);
/// ```
pub fn resample_audio(samples: &[f32], from_rate: usize, to_rate: usize) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = to_rate as f32 / from_rate as f32;
    let new_length = ((samples.len() as f32) * ratio).ceil() as usize;
    let mut resampled = Vec::with_capacity(new_length);

    for i in 0..new_length {
        let src_idx = i as f32 / ratio;
        let src_idx_floor = src_idx.floor() as usize;
        let src_idx_ceil = src_idx_floor + 1;
        let fraction = src_idx - (src_idx_floor as f32);

        let interpolated = if src_idx_ceil < samples.len() {
            // Linear interpolation
            samples[src_idx_floor] * (1.0 - fraction) + samples[src_idx_ceil] * fraction
        } else {
            samples[src_idx_floor]
        };

        resampled.push(interpolated);
    }

    resampled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stereo_to_mono() {
        let stereo = vec![0.2, 0.8, 0.4, 0.6]; // [L, R, L, R]
        let mono = stereo_to_mono(&stereo);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.5).abs() < 0.001); // (0.2 + 0.8) / 2 = 0.5
        assert!((mono[1] - 0.5).abs() < 0.001); // (0.4 + 0.6) / 2 = 0.5
    }

    #[test]
    fn test_stereo_to_mono_different_values() {
        let stereo = vec![0.1, 0.3, 0.5, 0.7, -0.2, -0.4];
        let mono = stereo_to_mono(&stereo);
        assert_eq!(mono.len(), 3);
        assert!((mono[0] - 0.2).abs() < 0.001);
        assert!((mono[1] - 0.6).abs() < 0.001);
        assert!((mono[2] - (-0.3)).abs() < 0.001);
    }

    #[test]
    fn test_resample_same_rate() {
        let samples = vec![0.1, 0.2, 0.3, 0.4];
        let resampled = resample_audio(&samples, 16000, 16000);
        assert_eq!(resampled, samples);
    }

    #[test]
    fn test_resample_downsample() {
        let samples = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        let resampled = resample_audio(&samples, 48000, 16000); // 3x downsample
        assert!(resampled.len() < samples.len());
        // Should have approximately 1/3 the samples
        assert!(resampled.len() >= (samples.len() / 3) - 1);
        assert!(resampled.len() <= (samples.len() / 3) + 1);
    }

    #[test]
    fn test_resample_upsample() {
        let samples = vec![0.1, 0.2, 0.3, 0.4];
        let resampled = resample_audio(&samples, 16000, 48000); // 3x upsample
        assert!(resampled.len() > samples.len());
        // Should have approximately 3x the samples
        assert!(resampled.len() >= (samples.len() * 3) - 2);
        assert!(resampled.len() <= (samples.len() * 3) + 2);
    }

    #[test]
    fn test_resample_preserves_value_range() {
        let samples = vec![0.1, 0.5, -0.3, 0.8, -0.2];
        let resampled = resample_audio(&samples, 16000, 22050);

        // Check that resampled values stay within reasonable range
        for sample in resampled {
            assert!(sample >= -1.1 && sample <= 1.1, "Sample out of range: {}", sample);
        }
    }
}
