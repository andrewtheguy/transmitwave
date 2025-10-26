use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioModemError {
    #[error("Failed to detect preamble")]
    PreambleNotFound,

    #[error("Failed to detect postamble")]
    PostambleNotFound,

    #[error("CRC mismatch in frame header")]
    HeaderCrcMismatch,

    #[error("CRC mismatch in frame payload")]
    PayloadCrcMismatch,

    #[error("Reed-Solomon decode failure")]
    FecDecodeFailure,

    #[error("Invalid frame size")]
    InvalidFrameSize,

    #[error("FFT error: {0}")]
    FftError(String),

    #[error("Invalid input size")]
    InvalidInputSize,

    #[error("Insufficient data")]
    InsufficientData,

    #[error("Frame number mismatch")]
    FrameNumberMismatch,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("FEC error: {0}")]
    FecError(String),

    #[error("Fountain decode failure")]
    FountainDecodeFailure,

    #[error("Operation timeout")]
    Timeout,
}

pub type Result<T> = std::result::Result<T, AudioModemError>;
