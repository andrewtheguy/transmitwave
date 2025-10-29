//! Audio modem library for reliable low-bandwidth communication
//!
//! Uses Multi-tone FSK (ggwave-compatible) with Reed-Solomon FEC
//! for maximum reliability in over-the-air audio transmission

pub mod error;
pub mod fec;
pub mod framing;
pub mod fft_correlation;
pub mod sync;
pub mod resample;
pub mod fsk;
pub mod dtmf;
pub mod encoder_fsk;
pub mod decoder_fsk;
pub mod encoder_dtmf;
pub mod decoder_dtmf;

pub use encoder_fsk::{EncoderFsk, FountainStream};
pub use decoder_fsk::DecoderFsk;
pub use encoder_dtmf::EncoderDtmf;
pub use decoder_dtmf::DecoderDtmf;
pub use error::{AudioModemError, Result};
pub use fft_correlation::{Mode, fft_correlate_1d};
pub use sync::{detect_preamble, detect_postamble, DetectionThreshold};
pub use resample::{resample_audio, stereo_to_mono};
pub use fec::{FecEncoder, FecDecoder};
pub use fsk::{FskModulator, FskDemodulator, FountainConfig};
pub use dtmf::{DtmfModulator, DtmfDemodulator, DTMF_NUM_SYMBOLS, DTMF_SYMBOL_SAMPLES};

// Configuration constants
pub const SAMPLE_RATE: usize = 16000;
pub const SYMBOL_DURATION_MS: usize = 100;
pub const SAMPLES_PER_SYMBOL: usize = (SAMPLE_RATE * SYMBOL_DURATION_MS) / 1000; // 1600

// FSK configuration (multi-tone for robustness)
// Uses 96 frequency bins with 6 simultaneous tones for non-coherent detection
// Optimized for mobile phone speakers (800-2700 Hz range)
pub const FSK_MIN_FREQUENCY: f32 = 800.0; // Hz (base frequency)
pub const FSK_MAX_FREQUENCY: f32 = 2700.0; // Hz (max frequency)
pub const NUM_FSK_TONES: usize = 6; // 6 simultaneous frequencies per symbol (3 bytes)

// Preamble/Postamble sync signal
pub const SYNC_DURATION_MS: usize = 250; // Preamble/postamble duration (1/4 second)
pub const PREAMBLE_DURATION_MS: usize = SYNC_DURATION_MS;
pub const PREAMBLE_SAMPLES: usize = (SAMPLE_RATE * SYNC_DURATION_MS) / 1000; // 4000
pub const POSTAMBLE_DURATION_MS: usize = SYNC_DURATION_MS;
pub const POSTAMBLE_SAMPLES: usize = (SAMPLE_RATE * SYNC_DURATION_MS) / 1000; // 4000

// Brief silence gaps for better frame detection (1/16 second each)
pub const SYNC_SILENCE_MS: usize = 63; // Silence before/after sync signals
pub const SYNC_SILENCE_SAMPLES: usize = (SAMPLE_RATE * SYNC_SILENCE_MS) / 1000; // 1008

// FEC configuration
// Reed-Solomon (255, 223) - can correct up to 16 byte errors per 255-byte block
pub const RS_DATA_BYTES: usize = 223;
pub const RS_TOTAL_BYTES: usize = 255;
pub const RS_ECC_BYTES: usize = RS_TOTAL_BYTES - RS_DATA_BYTES; // 32 byte error correction

// Frame configuration
pub const FRAME_HEADER_SIZE: usize = 8; // payload length (2) + frame number (2) + CRC-8 (1) + reserved (3)
pub const MAX_PAYLOAD_SIZE: usize = 200;
