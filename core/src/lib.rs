//! Audio modem library for reliable low-bandwidth communication
//!
//! Uses OFDM with multiple overlapping frequencies (0-4kHz) with Reed-Solomon FEC

pub mod error;
pub mod ofdm;
pub mod ofdm_cp;
pub mod fec;
pub mod framing;
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
pub mod encoder_chunked;
pub mod decoder_chunked;

pub use encoder::Encoder;
pub use encoder_cp::EncoderCp;
pub use decoder::Decoder;
pub use decoder_cp::DecoderCp;
pub use encoder_spread::EncoderSpread;
pub use decoder_spread::DecoderSpread;
pub use encoder_chunked::EncoderChunked;
pub use decoder_chunked::DecoderChunked;
pub use error::{AudioModemError, Result};
pub use sync::{detect_preamble, detect_postamble};
pub use spread::{SpreadSpectrumSpreader, SpreadSpectrumDespreader};
pub use trellis::{ConvolutionalEncoder, ViterbiDecoder};
pub use resample::{resample_audio, stereo_to_mono};
pub use chunking::{Chunk, ChunkEncoder, ChunkDecoder, interleave_chunks};
pub use ofdm::{OfdmModulator, OfdmDemodulator};
pub use fec::{FecEncoder, FecDecoder};

// Configuration constants
pub const SAMPLE_RATE: usize = 16000;
pub const SYMBOL_DURATION_MS: usize = 100;
pub const SAMPLES_PER_SYMBOL: usize = (SAMPLE_RATE * SYMBOL_DURATION_MS) / 1000; // 1600

// OFDM configuration
pub const NUM_SUBCARRIERS: usize = 48;
pub const SUBCARRIER_SPACING: f32 = 79.0; // Hz
pub const MIN_FREQUENCY: f32 = 400.0; // Hz (lower pitch, closer to chirp)
pub const MAX_FREQUENCY: f32 = 3200.0; // Hz (lower max, narrower band)

// FEC configuration
pub const RS_DATA_BYTES: usize = 223;
pub const RS_TOTAL_BYTES: usize = 255;
pub const RS_ECC_BYTES: usize = RS_TOTAL_BYTES - RS_DATA_BYTES; // 32

// Frame configuration
pub const PREAMBLE_DURATION_MS: usize = 250;
pub const PREAMBLE_SAMPLES: usize = (SAMPLE_RATE * PREAMBLE_DURATION_MS) / 1000; // 4000
pub const POSTAMBLE_DURATION_MS: usize = 250;
pub const POSTAMBLE_SAMPLES: usize = (SAMPLE_RATE * POSTAMBLE_DURATION_MS) / 1000; // 4000

pub const FRAME_HEADER_SIZE: usize = 8; // payload length (2) + frame number (2) + CRC-8 (1) + reserved (3)
pub const MAX_PAYLOAD_SIZE: usize = 200;
