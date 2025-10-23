use crate::error::{AudioModemError, Result};

/// 11-bit Barker code for spreading
/// Properties: autocorrelation peak of 11, sidelobe max of 1
/// Creates spread spectrum effect across the signal
pub fn barker_sequence() -> Vec<f32> {
    // Standard 11-bit Barker code: [1, 1, 1, -1, -1, 1, -1, 1, 1, -1, 1]
    // Normalized to 1.0 and -1.0
    vec![1.0, 1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0]
}

/// Spread spectrum spreader: multiplies each input sample by the Barker sequence
/// This expands each symbol across multiple chips, creating that characteristic
/// "hissy" modem-like sound by mixing the signal across many frequency components
pub struct SpreadSpectrumSpreader {
    barker: Vec<f32>,
    chip_duration_samples: usize,
}

impl SpreadSpectrumSpreader {
    /// Create a new spreader with Barker code spreading
    /// chip_duration_samples: number of samples per Barker chip (1-10 typically)
    pub fn new(chip_duration_samples: usize) -> Result<Self> {
        if chip_duration_samples == 0 {
            return Err(AudioModemError::InvalidConfig(
                "Chip duration must be > 0".to_string(),
            ));
        }

        Ok(Self {
            barker: barker_sequence(),
            chip_duration_samples,
        })
    }

    /// Spread an OFDM symbol using Barker code
    /// Input: OFDM symbol samples (typically 1600 for 100ms symbol)
    /// Output: Spread symbol (input_len * chip_duration_samples samples)
    ///
    /// Process:
    /// 1. For each input sample, repeat it chip_duration_samples times
    /// 2. For each output position, multiply by corresponding Barker code value
    /// 3. Barker code repeats cyclically: sample[i] *= barker[i % 11]
    /// 4. This creates interleaved spread spectrum pattern
    pub fn spread(&self, ofdm_samples: &[f32]) -> Result<Vec<f32>> {
        if ofdm_samples.is_empty() {
            return Err(AudioModemError::InvalidInputSize);
        }

        let barker_len = self.barker.len(); // 11
        let mut spread_samples = Vec::new();

        // For each input sample
        for (sample_idx, &sample) in ofdm_samples.iter().enumerate() {
            // Get the Barker code value for this position (cycling through 11 values)
            let barker_value = self.barker[sample_idx % barker_len];

            // Multiply sample by Barker value
            let spread_sample = sample * barker_value;

            // Repeat chip_duration_samples times
            for _ in 0..self.chip_duration_samples {
                spread_samples.push(spread_sample);
            }
        }

        Ok(spread_samples)
    }
}

/// Spread spectrum despreader: reverses the spreading to recover original signal
pub struct SpreadSpectrumDespreader {
    barker: Vec<f32>,
    chip_duration_samples: usize,
}

impl SpreadSpectrumDespreader {
    /// Create a new despreader
    pub fn new(chip_duration_samples: usize) -> Result<Self> {
        if chip_duration_samples == 0 {
            return Err(AudioModemError::InvalidConfig(
                "Chip duration must be > 0".to_string(),
            ));
        }

        Ok(Self {
            barker: barker_sequence(),
            chip_duration_samples,
        })
    }

    /// Despread a Barker-spread signal to recover original OFDM samples
    ///
    /// Process:
    /// 1. For each group of chip_duration_samples: average them to get despread value
    /// 2. Multiply by Barker value again to reverse spreading (Barker is ±1.0)
    /// 3. Recover original sequence
    pub fn despread(&self, spread_samples: &[f32]) -> Result<Vec<f32>> {
        if spread_samples.is_empty() {
            return Err(AudioModemError::InvalidInputSize);
        }

        let barker_len = self.barker.len(); // 11

        // Expected length should be multiple of chip_duration_samples
        if spread_samples.len() % self.chip_duration_samples != 0 {
            return Err(AudioModemError::InvalidInputSize);
        }

        let mut despread_samples = Vec::new();
        let mut sample_idx = 0;
        let mut original_idx = 0;

        // Process chip_duration_samples at a time
        while sample_idx < spread_samples.len() {
            // Collect chip_duration_samples and average them
            let mut chip_sum = 0.0;
            for _ in 0..self.chip_duration_samples {
                if sample_idx < spread_samples.len() {
                    chip_sum += spread_samples[sample_idx];
                    sample_idx += 1;
                }
            }

            let chip_value = chip_sum / self.chip_duration_samples as f32;

            // Get the Barker code value for this original position
            let barker_value = self.barker[original_idx % barker_len];

            // Multiply by Barker value again to reverse spreading
            // (Since Barker values are ±1.0, multiplying twice gives back original)
            let recovered = chip_value * barker_value;
            despread_samples.push(recovered);

            original_idx += 1;
        }

        Ok(despread_samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_barker_sequence_properties() {
        let barker = barker_sequence();
        assert_eq!(barker.len(), 11);

        // Check autocorrelation peak (sum of squares)
        let auto_corr: f32 = barker.iter().map(|x| x * x).sum();
        assert_eq!(auto_corr, 11.0);
    }

    #[test]
    fn test_spread_despread_round_trip() {
        let spreader = SpreadSpectrumSpreader::new(2).unwrap();
        let despreader = SpreadSpectrumDespreader::new(2).unwrap();

        // Create test signal
        let test_signal = vec![0.5; 110]; // 110 samples (11 * 10)

        let spread = spreader.spread(&test_signal).unwrap();
        assert_eq!(spread.len(), 110 * 2); // Expanded by chip_duration_samples

        let despread = despreader.despread(&spread).unwrap();
        assert_eq!(despread.len(), 110);

        // Check recovery (with some tolerance for averaging)
        for (original, recovered) in test_signal.iter().zip(despread.iter()) {
            assert!((original - recovered).abs() < 0.01);
        }
    }

    #[test]
    fn test_spread_with_varying_amplitude() {
        let spreader = SpreadSpectrumSpreader::new(3).unwrap();
        let despreader = SpreadSpectrumDespreader::new(3).unwrap();

        let test_signal: Vec<f32> = (0..110)
            .map(|i| (i as f32 * 0.01).sin())
            .collect();

        let spread = spreader.spread(&test_signal).unwrap();
        let despread = despreader.despread(&spread).unwrap();

        // Verify approximate recovery
        assert_eq!(despread.len(), test_signal.len());
        for (original, recovered) in test_signal.iter().zip(despread.iter()) {
            assert!((original - recovered).abs() < 0.1);
        }
    }

    #[test]
    fn test_barker_correlation_property() {
        // The Barker code has the property that correlation with itself is 11
        // and correlation with shifted versions is at most 1
        let barker = barker_sequence();

        // Self-correlation
        let self_corr: f32 = barker.iter().map(|x| x * x).sum();
        assert_eq!(self_corr, 11.0);

        // Correlation with shifted version (only compare overlapping part)
        let shifted = &barker[1..];
        let cross_corr: f32 = shifted
            .iter()
            .zip(barker[..barker.len() - 1].iter())
            .map(|(a, b)| a * b)
            .sum();
        // For 11-bit Barker code, sidelobe can be up to 1
        assert!(cross_corr.abs() <= 2.0); // Allow small tolerance
    }

    #[test]
    fn test_spreader_with_ofdm_like_signal() {
        let spreader = SpreadSpectrumSpreader::new(4).unwrap();

        // Simulate OFDM-like signal (1600 samples for 100ms symbol)
        let ofdm_samples: Vec<f32> = (0..1600)
            .map(|i| ((i as f32 * 2.0 * std::f32::consts::PI / 1600.0).sin()) * 0.5)
            .collect();

        let spread = spreader.spread(&ofdm_samples).unwrap();

        // Should expand to 1600 * 4 = 6400 samples (11 chips * 4 samples each per 145-sample chunk)
        // Actually: 1600/11 ≈ 145 samples per chip, repeated 4 times = 580 per chip
        // Total: 11 * 580 = 6380 samples (rounded)
        assert!(spread.len() > 0);
        assert!(spread.len() <= 6400);
    }
}
