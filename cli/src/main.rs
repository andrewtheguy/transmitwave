use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use clap::{Parser, Subcommand};
use hound::WavSpec;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;
use testaudio_core::{Decoder, Encoder, DecoderSpread, EncoderSpread, resample_audio, stereo_to_mono, SAMPLE_RATE};
use tower_http::cors::CorsLayer;
use base64::Engine;

#[derive(Serialize, Deserialize)]
struct EncodeRequest {
    data: String, // base64-encoded input data
    #[serde(default = "default_chip_duration")]
    chip_duration: usize,
}

fn default_chip_duration() -> usize {
    2
}

#[derive(Serialize, Deserialize)]
struct EncodeResponse {
    success: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    wav_base64: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct DecodeRequest {
    wav_base64: String, // base64-encoded WAV file
    #[serde(default = "default_chip_duration")]
    chip_duration: usize,
}

#[derive(Serialize, Deserialize)]
struct DecodeResponse {
    success: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,
}

#[derive(Parser)]
#[command(name = "testaudio")]
#[command(about = "Audio modem with spread spectrum for reliable low-bandwidth communication")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Input binary file (encode) or WAV file (decode)
    #[arg(value_name = "INPUT")]
    input: Option<PathBuf>,

    /// Output WAV file (encode) or binary file (decode)
    #[arg(value_name = "OUTPUT")]
    output: Option<PathBuf>,

    /// Operation mode: encode or decode (auto-detect by file extension if not specified)
    #[arg(short, long, value_name = "MODE")]
    mode: Option<String>,

    /// Chip duration for spread spectrum (samples per Barker chip, default: 2)
    #[arg(short, long, default_value = "2")]
    chip_duration: usize,

    /// Use legacy encoder/decoder without spread spectrum
    #[arg(long)]
    no_spread: bool,

    /// Start web server on port 8000
    #[arg(long)]
    server: bool,

    /// Port for web server (default: 8000)
    #[arg(long, default_value = "8000")]
    port: u16,
}

#[derive(Subcommand)]
enum Commands {
    /// Encode binary data to WAV audio file with spread spectrum
    Encode {
        /// Input binary file
        #[arg(value_name = "INPUT.BIN")]
        input: PathBuf,

        /// Output WAV file
        #[arg(value_name = "OUTPUT.WAV")]
        output: PathBuf,

        /// Chip duration (samples per Barker chip, default: 2)
        #[arg(short, long, default_value = "2")]
        chip_duration: usize,

        /// Use legacy encoder without spread spectrum
        #[arg(long)]
        no_spread: bool,
    },

    /// Decode WAV file to binary data with spread spectrum
    Decode {
        /// Input WAV file
        #[arg(value_name = "INPUT.WAV")]
        input: PathBuf,

        /// Output binary file
        #[arg(value_name = "OUTPUT.BIN")]
        output: PathBuf,

        /// Chip duration (samples per Barker chip, must match encoder)
        #[arg(short, long, default_value = "2")]
        chip_duration: usize,

        /// Use legacy decoder without spread spectrum
        #[arg(long)]
        no_spread: bool,
    },

    /// Start web server for encode/decode operations
    Server {
        /// Port to listen on (default: 8000)
        #[arg(short, long, default_value = "8000")]
        port: u16,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Check if web server should be started
    if cli.server {
        return start_web_server(cli.port);
    }

    // Handle subcommands
    if let Some(command) = cli.command {
        match command {
            Commands::Encode { input, output, chip_duration, no_spread } => {
                if no_spread {
                    encode_legacy_command(&input, &output)?
                } else {
                    encode_spread_command(&input, &output, chip_duration)?
                }
            }
            Commands::Decode { input, output, chip_duration, no_spread } => {
                if no_spread {
                    decode_legacy_command(&input, &output)?
                } else {
                    decode_spread_command(&input, &output, chip_duration)?
                }
            }
            Commands::Server { port } => {
                return start_web_server(port);
            }
        }
        return Ok(());
    }

    // Default: positional arguments with auto-detection
    if let (Some(input), Some(output)) = (cli.input, cli.output) {
        // Auto-detect operation based on file extension
        let mode = cli.mode.as_deref().unwrap_or_else(|| {
            match input.extension().and_then(|s| s.to_str()) {
                Some("bin") => "encode",
                Some("wav") => "decode",
                _ => {
                    eprintln!("Error: Cannot auto-detect mode. Please specify --mode encode or --mode decode");
                    std::process::exit(1);
                }
            }
        });

        if mode == "encode" || mode == "enc" {
            if cli.no_spread {
                encode_legacy_command(&input, &output)?
            } else {
                encode_spread_command(&input, &output, cli.chip_duration)?
            }
        } else if mode == "decode" || mode == "dec" {
            if cli.no_spread {
                decode_legacy_command(&input, &output)?
            } else {
                decode_spread_command(&input, &output, cli.chip_duration)?
            }
        } else {
            eprintln!("Error: Unknown mode '{}'. Use 'encode' or 'decode'", mode);
            std::process::exit(1);
        }
    } else {
        eprintln!("Error: No operation specified. Use 'testaudio --help' for usage");
        std::process::exit(1);
    }

    Ok(())
}

fn encode_legacy_command(input_path: &PathBuf, output_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Read input binary file
    let data = std::fs::read(input_path)?;
    println!("Read {} bytes from {}", data.len(), input_path.display());

    // Create encoder and encode data
    let mut encoder = Encoder::new()?;
    let samples = encoder.encode(&data)?;
    println!("Encoded (legacy, no spreading) to {} audio samples", samples.len());

    // Write WAV file (16-bit PCM)
    let spec = WavSpec {
        channels: 1,
        sample_rate: testaudio_core::SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let file = File::create(output_path)?;
    let mut writer = hound::WavWriter::new(file, spec)?;

    // Convert f32 samples to i16 range [-32768, 32767]
    for sample in samples {
        // Clamp to [-1.0, 1.0] range to avoid overflow, then scale to i16
        let clamped = sample.max(-1.0).min(1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        writer.write_sample(i16_sample)?;
    }
    writer.finalize()?;

    println!("Wrote {} to {}", output_path.display(), spec.channels);
    Ok(())
}

fn decode_legacy_command(input_path: &PathBuf, output_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Read WAV file
    let file = File::open(input_path)?;
    let mut reader = hound::WavReader::new(file)?;

    let spec = reader.spec();
    println!(
        "Read WAV: {} Hz, {} channels, {} bits",
        spec.sample_rate, spec.channels, spec.bits_per_sample
    );

    // Extract samples (handle both 16-bit and 32-bit float formats)
    let mut samples = match spec.bits_per_sample {
        16 => {
            // Convert i16 to f32
            let int_samples: Result<Vec<i16>, _> = reader.samples::<i16>().collect();
            int_samples?
                .into_iter()
                .map(|s| s as f32 / 32768.0)
                .collect()
        }
        32 => {
            // Already f32
            let float_samples: Result<Vec<f32>, _> = reader.samples::<f32>().collect();
            float_samples?
        }
        _ => {
            return Err(format!("Unsupported bit depth: {}", spec.bits_per_sample).into());
        }
    };

    println!("Extracted {} samples", samples.len());

    // Convert to mono if stereo
    if spec.channels == 2 {
        println!("Converting stereo to mono...");
        samples = stereo_to_mono(&samples);
        println!("Converted to {} mono samples", samples.len());
    }

    // Resample to 16kHz if needed
    if spec.sample_rate != SAMPLE_RATE as u32 {
        println!("Resampling from {} Hz to {} Hz...", spec.sample_rate, SAMPLE_RATE);
        samples = resample_audio(&samples, spec.sample_rate as usize, SAMPLE_RATE);
        println!("Resampled to {} samples", samples.len());
    }

    // Decode
    let mut decoder = Decoder::new()?;
    let data = decoder.decode(&samples)?;
    println!("Decoded {} bytes", data.len());

    // Write binary file
    std::fs::write(output_path, &data)?;
    println!("Wrote {} to {}", data.len(), output_path.display());

    Ok(())
}

fn encode_spread_command(
    input_path: &PathBuf,
    output_path: &PathBuf,
    chip_duration: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read input binary file
    let data = std::fs::read(input_path)?;
    println!("Read {} bytes from {}", data.len(), input_path.display());

    // Create encoder and encode data
    let mut encoder = EncoderSpread::new(chip_duration)?;
    let samples = encoder.encode(&data)?;
    println!(
        "Encoded with spread spectrum (chip_duration={}) to {} audio samples",
        chip_duration,
        samples.len()
    );

    // Write WAV file (16-bit PCM)
    let spec = WavSpec {
        channels: 1,
        sample_rate: testaudio_core::SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let file = File::create(output_path)?;
    let mut writer = hound::WavWriter::new(file, spec)?;

    // Convert f32 samples to i16 range [-32768, 32767]
    for sample in samples {
        // Clamp to [-1.0, 1.0] range to avoid overflow, then scale to i16
        let clamped = sample.max(-1.0).min(1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        writer.write_sample(i16_sample)?;
    }
    writer.finalize()?;

    println!("Wrote {} to {}", output_path.display(), spec.channels);
    Ok(())
}

fn decode_spread_command(
    input_path: &PathBuf,
    output_path: &PathBuf,
    chip_duration: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read WAV file
    let file = File::open(input_path)?;
    let mut reader = hound::WavReader::new(file)?;

    let spec = reader.spec();
    println!(
        "Read WAV: {} Hz, {} channels, {} bits",
        spec.sample_rate, spec.channels, spec.bits_per_sample
    );

    // Extract samples (handle both 16-bit and 32-bit float formats)
    let mut samples = match spec.bits_per_sample {
        16 => {
            // Convert i16 to f32
            let int_samples: Result<Vec<i16>, _> = reader.samples::<i16>().collect();
            int_samples?
                .into_iter()
                .map(|s| s as f32 / 32768.0)
                .collect()
        }
        32 => {
            // Already f32
            let float_samples: Result<Vec<f32>, _> = reader.samples::<f32>().collect();
            float_samples?
        }
        _ => {
            return Err(format!("Unsupported bit depth: {}", spec.bits_per_sample).into());
        }
    };

    println!("Extracted {} samples", samples.len());

    // Convert to mono if stereo
    if spec.channels == 2 {
        println!("Converting stereo to mono...");
        samples = stereo_to_mono(&samples);
        println!("Converted to {} mono samples", samples.len());
    }

    // Resample to 16kHz if needed
    if spec.sample_rate != SAMPLE_RATE as u32 {
        println!("Resampling from {} Hz to {} Hz...", spec.sample_rate, SAMPLE_RATE);
        samples = resample_audio(&samples, spec.sample_rate as usize, SAMPLE_RATE);
        println!("Resampled to {} samples", samples.len());
    }

    // Decode with spread spectrum
    let mut decoder = DecoderSpread::new(chip_duration)?;
    let data = decoder.decode(&samples)?;
    println!("Decoded {} bytes (chip_duration={})", data.len(), chip_duration);

    // Write binary file
    std::fs::write(output_path, &data)?;
    println!("Wrote {} to {}", data.len(), output_path.display());

    Ok(())
}

#[tokio::main]
async fn start_web_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting testaudio server on http://localhost:{}", port);
    println!("Endpoints:");
    println!("  POST /encode - Encode binary data to WAV with spread spectrum");
    println!("  POST /decode - Decode WAV to binary data with spread spectrum");
    println!("  GET / - Server status");

    let app = Router::new()
        .route("/", get(handler_status))
        .route("/encode", post(handler_encode))
        .route("/decode", post(handler_decode))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handler_status() -> &'static str {
    "testaudio server with spread spectrum encoding/decoding - Ready"
}

async fn handler_encode(
    Json(req): Json<EncodeRequest>,
) -> Result<Json<EncodeResponse>, (StatusCode, Json<EncodeResponse>)> {
    let data = base64::engine::general_purpose::STANDARD
        .decode(&req.data)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(EncodeResponse {
                    success: false,
                    message: format!("Invalid base64 data: {}", e),
                    wav_base64: None,
                }),
            )
        })?;

    if data.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(EncodeResponse {
                success: false,
                message: "No data provided".to_string(),
                wav_base64: None,
            }),
        ));
    }

    match EncoderSpread::new(req.chip_duration) {
        Ok(mut encoder) => match encoder.encode(&data) {
            Ok(samples) => {
                // Convert to WAV
                let spec = WavSpec {
                    channels: 1,
                    sample_rate: testaudio_core::SAMPLE_RATE as u32,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                };

                let wav_data_result = {
                    let mut wav_data = Vec::new();
                    {
                        let cursor = std::io::Cursor::new(&mut wav_data);
                        match hound::WavWriter::new(cursor, spec) {
                            Ok(mut writer) => {
                                for sample in samples {
                                    let clamped = sample.max(-1.0).min(1.0);
                                    let i16_sample = (clamped * 32767.0) as i16;
                                    if writer.write_sample(i16_sample).is_err() {
                                        return Err((
                                            StatusCode::INTERNAL_SERVER_ERROR,
                                            Json(EncodeResponse {
                                                success: false,
                                                message: "Failed to write WAV samples".to_string(),
                                                wav_base64: None,
                                            }),
                                        ));
                                    }
                                }
                                if writer.finalize().is_err() {
                                    return Err((
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(EncodeResponse {
                                            success: false,
                                            message: "Failed to finalize WAV".to_string(),
                                            wav_base64: None,
                                        }),
                                    ));
                                }
                            }
                            Err(e) => {
                                return Err((
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(EncodeResponse {
                                        success: false,
                                        message: format!("Failed to create WAV: {}", e),
                                        wav_base64: None,
                                    }),
                                ));
                            }
                        }
                    }
                    wav_data
                };

                let wav_base64 = base64::engine::general_purpose::STANDARD.encode(&wav_data_result);
                Ok(Json(EncodeResponse {
                    success: true,
                    message: format!(
                        "Encoded {} bytes with chip_duration={} to {} samples",
                        data.len(),
                        req.chip_duration,
                        (wav_data_result.len() - 44) / 2
                    ),
                    wav_base64: Some(wav_base64),
                }))
            }
            Err(e) => Err((
                StatusCode::BAD_REQUEST,
                Json(EncodeResponse {
                    success: false,
                    message: format!("Encoding failed: {}", e),
                    wav_base64: None,
                }),
            )),
        },
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(EncodeResponse {
                success: false,
                message: format!("Failed to create encoder: {}", e),
                wav_base64: None,
            }),
        )),
    }
}

async fn handler_decode(
    Json(req): Json<DecodeRequest>,
) -> Result<Json<DecodeResponse>, (StatusCode, Json<DecodeResponse>)> {
    let wav_data = base64::engine::general_purpose::STANDARD
        .decode(&req.wav_base64)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(DecodeResponse {
                    success: false,
                    message: format!("Invalid base64 WAV data: {}", e),
                    data: None,
                }),
            )
        })?;

    if wav_data.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(DecodeResponse {
                success: false,
                message: "No WAV data provided".to_string(),
                data: None,
            }),
        ));
    }

    // Parse WAV file
    let cursor = std::io::Cursor::new(&wav_data);
    match hound::WavReader::new(cursor) {
        Ok(mut reader) => {
            let spec = reader.spec();
            let samples: Vec<f32> = match spec.bits_per_sample {
                16 => {
                    match reader.samples::<i16>().collect::<Result<Vec<_>, _>>() {
                        Ok(int_samples) => int_samples.iter().map(|s| *s as f32 / 32768.0).collect(),
                        Err(e) => {
                            return Err((
                                StatusCode::BAD_REQUEST,
                                Json(DecodeResponse {
                                    success: false,
                                    message: format!("Failed to read i16 samples: {}", e),
                                    data: None,
                                }),
                            ));
                        }
                    }
                }
                32 => {
                    match reader.samples::<f32>().collect::<Result<Vec<_>, _>>() {
                        Ok(samples) => samples,
                        Err(e) => {
                            return Err((
                                StatusCode::BAD_REQUEST,
                                Json(DecodeResponse {
                                    success: false,
                                    message: format!("Failed to read f32 samples: {}", e),
                                    data: None,
                                }),
                            ));
                        }
                    }
                }
                _ => {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(DecodeResponse {
                            success: false,
                            message: format!("Unsupported bit depth: {}", spec.bits_per_sample),
                            data: None,
                        }),
                    ));
                }
            };

            match DecoderSpread::new(req.chip_duration) {
                Ok(mut decoder) => match decoder.decode(&samples) {
                    Ok(decoded_data) => {
                        let data_base64 = base64::engine::general_purpose::STANDARD.encode(&decoded_data);
                        Ok(Json(DecodeResponse {
                            success: true,
                            message: format!(
                                "Decoded {} bytes with chip_duration={}",
                                decoded_data.len(),
                                req.chip_duration
                            ),
                            data: Some(data_base64),
                        }))
                    }
                    Err(e) => Err((
                        StatusCode::BAD_REQUEST,
                        Json(DecodeResponse {
                            success: false,
                            message: format!("Decoding failed: {}", e),
                            data: None,
                        }),
                    )),
                },
                Err(e) => Err((
                    StatusCode::BAD_REQUEST,
                    Json(DecodeResponse {
                        success: false,
                        message: format!("Failed to create decoder: {}", e),
                        data: None,
                    }),
                )),
            }
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(DecodeResponse {
                success: false,
                message: format!("Failed to read WAV: {}", e),
                data: None,
            }),
        )),
    }
}
