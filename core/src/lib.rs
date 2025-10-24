//! Audio modem library for reliable low-bandwidth communication
//!
//! Uses OFDM with multiple overlapping frequencies (0-4kHz) with Reed-Solomon FEC

pub mod error;
pub mod ofdm;
pub mod ofdm_cp;
pub mod fec;
pub mod framing;
pub mod fft_correlation;
pub mod sync;
pub mod encoder;
pub mod encoder_cp;
pub mod decoder;
pub mod decoder_cp;
pub mod encoder_spread;
pub mod decoder_spread;
pub mod spread;
pub mod trellis;
pub mod resample;
pub mod chunking;
pub mod fsk;
pub mod encoder_fsk;
pub mod decoder_fsk;

pub use encoder::Encoder;
pub use encoder_cp::EncoderCp;
pub use decoder::Decoder;
pub use decoder_cp::DecoderCp;
pub use encoder_spread::EncoderSpread;
pub use decoder_spread::DecoderSpread;
pub use encoder_fsk::EncoderFsk;
pub use decoder_fsk::DecoderFsk;
pub use error::{AudioModemError, Result};
pub use fft_correlation::{Mode, fft_correlate_1d};
pub use sync::{detect_preamble, detect_postamble};
pub use spread::{SpreadSpectrumSpreader, SpreadSpectrumDespreader};
pub use trellis::{ConvolutionalEncoder, ViterbiDecoder};
pub use resample::{resample_audio, stereo_to_mono};
pub use chunking::{Chunk, ChunkEncoder, ChunkDecoder, interleave_chunks};
pub use ofdm::{OfdmModulator, OfdmDemodulator};
pub use fec::{FecEncoder, FecDecoder};
pub use fsk::{FskModulator, FskDemodulator};

// Configuration constants
pub const SAMPLE_RATE: usize = 16000;
pub const SYMBOL_DURATION_MS: usize = 100;
pub const SAMPLES_PER_SYMBOL: usize = (SAMPLE_RATE * SYMBOL_DURATION_MS) / 1000; // 1600

// OFDM configuration for DATA portion (raised pitch)
// Reduced to 96 subcarriers for improved reliability over speed
// Higher bitrate (224) caused too many bit errors, exceeding RS-FEC correction capacity
// 96 subcarriers = better SNR per carrier, lower BER, higher payload CRC success rate
// Each subcarrier gets a deterministic phase offset for phase randomization
pub const NUM_SUBCARRIERS: usize = 96;
pub const MIN_FREQUENCY: f32 = 1500.0; // Hz - raised for data portion
pub const MAX_FREQUENCY: f32 = 4000.0; // Hz - raised for data portion
// FFT bin resolution: 16000 Hz / 1600 samples = 10 Hz per bin
pub const BIN_RESOLUTION_HZ: f32 = 10.0; // SAMPLE_RATE / SAMPLES_PER_SYMBOL
pub const MIN_BIN: usize = 150; // 1500 Hz / 10 Hz per bin = bin index 150
pub const MAX_BIN: usize = 400; // 4000 Hz / 10 Hz per bin = bin index 400
// Subcarriers are uniformly distributed across FFT bins [MIN_BIN, MAX_BIN]
// using compute_carrier_bins() to ensure proper alignment on the 10 Hz grid
// OFDM amplitude normalization target to prevent clipping across all symbols
pub const OFDM_TARGET_AMPLITUDE: f32 = 0.7;

// Preamble/Postamble PN sync signal frequencies (kept low)
pub const PN_MIN_FREQUENCY: f32 = 400.0; // Hz
pub const PN_MAX_FREQUENCY: f32 = 3200.0; // Hz

// FEC configuration
// Reed-Solomon (255, 223) - can correct up to 16 byte errors per 255-byte block
// With 96 subcarriers: ~9.6 bits per symbol, max 5 symbols per RS block = ~48 bits
// This provides sufficient correction headroom to keep payload CRC pass rate high
pub const RS_DATA_BYTES: usize = 223;
pub const RS_TOTAL_BYTES: usize = 255;
pub const RS_ECC_BYTES: usize = RS_TOTAL_BYTES - RS_DATA_BYTES; // 32 byte error correction

// Frame configuration
pub const SYNC_DURATION_MS: usize = 250;  // Preamble/postamble duration (1/4 second)
pub const PREAMBLE_DURATION_MS: usize = SYNC_DURATION_MS;
pub const PREAMBLE_SAMPLES: usize = (SAMPLE_RATE * SYNC_DURATION_MS) / 1000; // 4000
pub const POSTAMBLE_DURATION_MS: usize = SYNC_DURATION_MS;
pub const POSTAMBLE_SAMPLES: usize = (SAMPLE_RATE * SYNC_DURATION_MS) / 1000; // 4000

pub const FRAME_HEADER_SIZE: usize = 8; // payload length (2) + frame number (2) + CRC-8 (1) + reserved (3)
pub const MAX_PAYLOAD_SIZE: usize = 200;

/// Precompute uniform carrier bin positions across the FFT grid
/// Maps NUM_SUBCARRIERS uniformly across [MIN_BIN, MAX_BIN]
pub fn compute_carrier_bins() -> [usize; NUM_SUBCARRIERS] {
    let mut bins = [0usize; NUM_SUBCARRIERS];
    for i in 0..NUM_SUBCARRIERS {
        let normalized = i as f32 / ((NUM_SUBCARRIERS - 1) as f32);
        let bin_float = MIN_BIN as f32 + normalized * ((MAX_BIN - MIN_BIN) as f32);
        bins[i] = bin_float.round() as usize;
    }
    bins
}
