//! Frequency-Hopping Spread Spectrum (FHSS) module
//!
//! Provides pseudorandom hopping patterns across multiple frequency bands
//! to improve resistance to narrowband interference and jamming.

use crate::error::{AudioModemError, Result};

/// Get the frequency band for a given symbol index
/// Uses a simple LFSR-based deterministic pseudorandom sequence
///
/// # Arguments
/// * `symbol_index` - Symbol number (0, 1, 2, ...)
/// * `num_bands` - Number of frequency bands (2-4)
///
/// # Returns
/// Band index (0 to num_bands-1)
pub fn get_band_for_symbol(symbol_index: usize, num_bands: usize) -> usize {
    if num_bands == 0 || num_bands > 4 {
        return 0;
    }

    if num_bands == 1 {
        return 0;
    }

    // Simple LFSR-based pseudorandom sequence
    // 16-bit LFSR with polynomial 0x8016 (provides good distribution)
    let mut lfsr = (symbol_index as u16).wrapping_add(0x6359);

    for _ in 0..8 {
        let lsb = lfsr & 1;
        lfsr >>= 1;
        if lsb == 1 {
            lfsr ^= 0x8016; // XOR with polynomial
        }
    }

    (lfsr as usize) % num_bands
}

/// Get the frequency range for a given band
///
/// # Arguments
/// * `band_index` - Band number (0-3)
/// * `num_bands` - Total number of bands
///
/// # Returns
/// Tuple of (min_freq_hz, max_freq_hz)
pub fn get_band_frequencies(band_index: usize, num_bands: usize) -> Result<(f32, f32)> {
    if num_bands == 0 || num_bands > 4 {
        return Err(AudioModemError::InvalidConfig(
            "Number of bands must be 1-4".to_string(),
        ));
    }

    if band_index >= num_bands {
        return Err(AudioModemError::InvalidConfig(
            format!("Band index {} out of range for {} bands", band_index, num_bands),
        ));
    }

    // Base frequency range: 400-3200 Hz (2800 Hz bandwidth)
    let min_base = 400.0;
    let max_base = 3200.0;
    let total_bandwidth = max_base - min_base;

    match num_bands {
        1 => {
            // Single band: use full range
            Ok((min_base, max_base))
        }
        2 => {
            // Two bands: split equally
            let band_width = total_bandwidth / 2.0;
            let min_freq = min_base + (band_index as f32) * band_width;
            let max_freq = min_freq + band_width;
            Ok((min_freq, max_freq))
        }
        3 => {
            // Three bands
            let band_width = total_bandwidth / 3.0;
            let min_freq = min_base + (band_index as f32) * band_width;
            let max_freq = min_freq + band_width;
            Ok((min_freq, max_freq))
        }
        4 => {
            // Four bands
            let band_width = total_bandwidth / 4.0;
            let min_freq = min_base + (band_index as f32) * band_width;
            let max_freq = min_freq + band_width;
            Ok((min_freq, max_freq))
        }
        _ => Err(AudioModemError::InvalidConfig(
            "Unsupported number of bands".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_band_for_symbol_deterministic() {
        // Same symbol index should always return same band
        let band1 = get_band_for_symbol(5, 3);
        let band2 = get_band_for_symbol(5, 3);
        assert_eq!(band1, band2);
    }

    #[test]
    fn test_get_band_for_symbol_different_indices() {
        // Different indices should (usually) give different bands
        let bands: Vec<_> = (0..10)
            .map(|i| get_band_for_symbol(i, 3))
            .collect();

        // At least 2 different bands should be used
        let unique_bands: std::collections::HashSet<_> = bands.into_iter().collect();
        assert!(unique_bands.len() >= 2);
    }

    #[test]
    fn test_get_band_for_symbol_within_range() {
        for i in 0..100 {
            let band = get_band_for_symbol(i, 3);
            assert!(band < 3);
        }
    }

    #[test]
    fn test_get_band_frequencies_single_band() {
        let (min, max) = get_band_frequencies(0, 1).unwrap();
        assert_eq!(min, 400.0);
        assert_eq!(max, 3200.0);
    }

    #[test]
    fn test_get_band_frequencies_two_bands() {
        let (min0, max0) = get_band_frequencies(0, 2).unwrap();
        let (min1, max1) = get_band_frequencies(1, 2).unwrap();

        assert_eq!(min0, 400.0);
        assert_eq!(max0, 1800.0);
        assert_eq!(min1, 1800.0);
        assert_eq!(max1, 3200.0);
    }

    #[test]
    fn test_get_band_frequencies_three_bands() {
        let (min0, max0) = get_band_frequencies(0, 3).unwrap();
        let (min1, max1) = get_band_frequencies(1, 3).unwrap();
        let (min2, max2) = get_band_frequencies(2, 3).unwrap();

        // Check coverage and no overlap
        assert_eq!(min0, 400.0);
        assert!(max0 > min0);
        assert_eq!(min1, max0);
        assert!(max1 > min1);
        assert_eq!(min2, max1);
        assert!(max2 > min2);
        assert!(max2 <= 3200.0);
    }

    #[test]
    fn test_get_band_frequencies_four_bands() {
        for band_idx in 0..4 {
            let result = get_band_frequencies(band_idx, 4);
            assert!(result.is_ok());
            let (min, max) = result.unwrap();
            assert!(min >= 400.0);
            assert!(max <= 3200.0);
            assert!(max > min);
        }
    }

    #[test]
    fn test_get_band_frequencies_invalid_band_index() {
        let result = get_band_frequencies(3, 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_band_frequencies_invalid_num_bands() {
        let result = get_band_frequencies(0, 5);
        assert!(result.is_err());
    }
}
