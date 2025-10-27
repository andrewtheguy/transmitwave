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
use transmitwave_core::{DecoderFsk, EncoderFsk, FountainConfig, resample_audio, stereo_to_mono, SAMPLE_RATE};
use tower_http::cors::CorsLayer;
use base64::Engine;

// ============================================================================
// ENCODER/DECODER CONFIGURATION
// Mode: Multi-tone FSK (ggwave-compatible) for maximum reliability
// This is the only supported mode for over-the-air audio transfer
// ============================================================================

#[derive(Serialize, Deserialize)]
struct EncodeRequest {
    data: String, // base64-encoded input data
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
}

#[derive(Serialize, Deserialize)]
struct DecodeResponse {
    success: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,
}

#[derive(Parser)]
#[command(name = "transmitwave")]
#[command(about = "Audio modem using multi-tone FSK for reliable over-the-air communication")]
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

    /// Start web server on port 8000
    #[arg(long)]
    server: bool,

    /// Port for web server (default: 8000)
    #[arg(long, default_value = "8000")]
    port: u16,
}

#[derive(Subcommand)]
enum Commands {
    /// Encode binary data to WAV audio file using multi-tone FSK
    Encode {
        /// Input binary file
        #[arg(value_name = "INPUT.BIN")]
        input: PathBuf,

        /// Output WAV file
        #[arg(value_name = "OUTPUT.WAV")]
        output: PathBuf,
    },

    /// Decode WAV file to binary data using multi-tone FSK
    Decode {
        /// Input WAV file
        #[arg(value_name = "INPUT.WAV")]
        input: PathBuf,

        /// Output binary file
        #[arg(value_name = "OUTPUT.BIN")]
        output: PathBuf,

        /// Detection threshold for both preamble and postamble (0.0=adaptive, 0.1-1.0=fixed)
        #[arg(short, long)]
        threshold: Option<f32>,

        /// Detection threshold for preamble only (overrides --threshold for preamble)
        #[arg(long)]
        preamble_threshold: Option<f32>,

        /// Detection threshold for postamble only (overrides --threshold for postamble)
        #[arg(long)]
        postamble_threshold: Option<f32>,
    },

    /// Start web server for encode/decode operations
    Server {
        /// Port to listen on (default: 8000)
        #[arg(short, long, default_value = "8000")]
        port: u16,
    },

    /// Encode binary data to WAV using fountain mode (continuous streaming)
    FountainEncode {
        /// Input binary file
        #[arg(value_name = "INPUT.BIN")]
        input: PathBuf,

        /// Output WAV file
        #[arg(value_name = "OUTPUT.WAV")]
        output: PathBuf,

        /// Timeout in seconds (default: 30)
        #[arg(short, long, default_value = "30")]
        timeout: u32,

        /// Block size in bytes (default: 64)
        #[arg(short, long, default_value = "64")]
        block_size: usize,

        /// Repair blocks ratio (default: 0.5)
        #[arg(short, long, default_value = "0.5")]
        repair_ratio: f32,
    },

    /// Decode WAV file using fountain mode
    FountainDecode {
        /// Input WAV file
        #[arg(value_name = "INPUT.WAV")]
        input: PathBuf,

        /// Output binary file
        #[arg(value_name = "OUTPUT.BIN")]
        output: PathBuf,

        /// Timeout in seconds (default: 30)
        #[arg(short, long, default_value = "30")]
        timeout: u32,

        /// Block size in bytes (default: 64)
        #[arg(short, long, default_value = "64")]
        block_size: usize,

        /// Detection threshold for both preamble and postamble (0.0=adaptive, 0.1-1.0=fixed)
        #[arg(long)]
        threshold: Option<f32>,

        /// Detection threshold for preamble only (overrides --threshold for preamble)
        #[arg(long)]
        preamble_threshold: Option<f32>,

        /// Detection threshold for postamble only (overrides --threshold for postamble)
        #[arg(long)]
        postamble_threshold: Option<f32>,
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
            Commands::Encode { input, output } => {
                encode_fsk_command(&input, &output)?
            }
            Commands::Decode { input, output, threshold, preamble_threshold, postamble_threshold } => {
                decode_fsk_command(&input, &output, threshold, preamble_threshold, postamble_threshold)?
            }
            Commands::Server { port } => {
                return start_web_server(port);
            }
            Commands::FountainEncode { input, output, timeout, block_size, repair_ratio } => {
                fountain_encode_command(&input, &output, timeout, block_size, repair_ratio)?
            }
            Commands::FountainDecode { input, output, timeout, block_size, threshold, preamble_threshold, postamble_threshold } => {
                fountain_decode_command(&input, &output, timeout, block_size, threshold, preamble_threshold, postamble_threshold)?
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
            encode_fsk_command(&input, &output)?
        } else if mode == "decode" || mode == "dec" {
            decode_fsk_command(&input, &output, None, None, None)?
        } else {
            eprintln!("Error: Unknown mode '{}'. Use 'encode' or 'decode'", mode);
            std::process::exit(1);
        }
    } else {
        eprintln!("Error: No operation specified. Use 'transmitwave --help' for usage");
        std::process::exit(1);
    }

    Ok(())
}

fn encode_fsk_command(
    input_path: &PathBuf,
    output_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read input binary file
    let data = std::fs::read(input_path)?;
    println!("Read {} bytes from {}", data.len(), input_path.display());

    // Create FSK encoder and encode data
    let mut encoder = EncoderFsk::new()?;
    let samples = encoder.encode(&data)?;
    println!(
        "Encoded with multi-tone FSK to {} audio samples",
        samples.len()
    );

    // Write WAV file (16-bit PCM)
    let spec = WavSpec {
        channels: 1,
        sample_rate: transmitwave_core::SAMPLE_RATE as u32,
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

fn fountain_encode_command(
    input_path: &PathBuf,
    output_path: &PathBuf,
    timeout: u32,
    block_size: usize,
    repair_ratio: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read input binary file
    let data = std::fs::read(input_path)?;
    println!("Read {} bytes from {}", data.len(), input_path.display());

    // Create fountain config
    let config = FountainConfig {
        timeout_secs: timeout,
        block_size,
        repair_blocks_ratio: repair_ratio,
    };

    println!(
        "Fountain mode: timeout={}s, block_size={}, repair_ratio={}",
        config.timeout_secs, config.block_size, config.repair_blocks_ratio
    );

    // Create FSK encoder and get fountain stream
    let mut encoder = EncoderFsk::new()?;
    let stream = encoder.encode_fountain(&data, Some(config))?;

    // Collect all blocks generated within timeout
    println!("Generating fountain blocks (this will take up to {} seconds)...", timeout);
    let mut all_samples = Vec::new();
    let mut block_count = 0;

    for block_samples in stream {
        all_samples.extend_from_slice(&block_samples);
        block_count += 1;
        if block_count % 10 == 0 {
            print!(".");
            use std::io::Write;
            std::io::stdout().flush()?;
        }
    }
    println!();
    println!("Generated {} fountain blocks ({} total samples)", block_count, all_samples.len());

    // Write WAV file (16-bit PCM)
    let spec = WavSpec {
        channels: 1,
        sample_rate: transmitwave_core::SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let file = File::create(output_path)?;
    let mut writer = hound::WavWriter::new(file, spec)?;

    // Convert f32 samples to i16 range [-32768, 32767]
    for sample in &all_samples {
        let clamped = sample.max(-1.0).min(1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        writer.write_sample(i16_sample)?;
    }
    writer.finalize()?;

    println!("Wrote fountain-encoded audio to {}", output_path.display());
    println!("Duration: {:.2}s", all_samples.len() as f32 / SAMPLE_RATE as f32);
    Ok(())
}

fn fountain_decode_command(
    input_path: &PathBuf,
    output_path: &PathBuf,
    timeout: u32,
    block_size: usize,
    threshold: Option<f32>,
    preamble_threshold: Option<f32>,
    postamble_threshold: Option<f32>,
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
            let int_samples: Result<Vec<i16>, _> = reader.samples::<i16>().collect();
            int_samples?
                .into_iter()
                .map(|s| s as f32 / 32768.0)
                .collect()
        }
        32 => {
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

    // Create fountain config
    let config = FountainConfig {
        timeout_secs: timeout,
        block_size,
        repair_blocks_ratio: 0.5, // Not used in decoder
    };

    println!(
        "Decoding with fountain mode (timeout={}s, block_size={})...",
        config.timeout_secs, config.block_size
    );

    // Decode with fountain mode
    let mut decoder = DecoderFsk::new()?;

    // Set detection thresholds with fallback logic:
    // - If specific threshold is provided, use it
    // - Otherwise, use the general --threshold if provided
    // - If nothing provided, use adaptive threshold (default)
    let actual_preamble = preamble_threshold.or(threshold);
    let actual_postamble = postamble_threshold.or(threshold);

    if let Some(thresh) = actual_preamble {
        decoder.set_preamble_threshold(thresh);
        if thresh < 1e-9 {
            println!("Using adaptive preamble detection threshold (auto-adjust based on signal)");
        } else {
            println!("Using fixed preamble detection threshold: {:.2}", thresh);
        }
    } else {
        println!("Using adaptive preamble detection threshold (auto-adjust based on signal)");
    }
    if let Some(thresh) = actual_postamble {
        decoder.set_postamble_threshold(thresh);
        if thresh < 1e-9 {
            println!("Using adaptive postamble detection threshold (auto-adjust based on signal)");
        } else {
            println!("Using fixed postamble detection threshold: {:.2}", thresh);
        }
    } else {
        println!("Using adaptive postamble detection threshold (auto-adjust based on signal)");
    }

    let data = decoder.decode_fountain(&samples, Some(config))?;
    println!("Successfully decoded {} bytes using fountain mode", data.len());

    // Write binary file
    std::fs::write(output_path, &data)?;
    println!("Wrote {} to {}", data.len(), output_path.display());

    Ok(())
}

fn decode_fsk_command(
    input_path: &PathBuf,
    output_path: &PathBuf,
    threshold: Option<f32>,
    preamble_threshold: Option<f32>,
    postamble_threshold: Option<f32>,
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

    // Decode with FSK
    let mut decoder = DecoderFsk::new()?;

    // Set detection thresholds with fallback logic:
    // - If specific threshold is provided, use it
    // - Otherwise, use the general --threshold if provided
    // - If nothing provided, use adaptive threshold (default)
    let actual_preamble = preamble_threshold.or(threshold);
    let actual_postamble = postamble_threshold.or(threshold);

    if let Some(thresh) = actual_preamble {
        decoder.set_preamble_threshold(thresh);
        if thresh < 1e-9 {
            println!("Using adaptive preamble detection threshold (auto-adjust based on signal)");
        } else {
            println!("Using fixed preamble detection threshold: {:.2}", thresh);
        }
    } else {
        println!("Using adaptive preamble detection threshold (auto-adjust based on signal)");
    }
    if let Some(thresh) = actual_postamble {
        decoder.set_postamble_threshold(thresh);
        if thresh < 1e-9 {
            println!("Using adaptive postamble detection threshold (auto-adjust based on signal)");
        } else {
            println!("Using fixed postamble detection threshold: {:.2}", thresh);
        }
    } else {
        println!("Using adaptive postamble detection threshold (auto-adjust based on signal)");
    }

    let data = decoder.decode(&samples)?;
    println!("Decoded {} bytes with multi-tone FSK", data.len());

    // Write binary file
    std::fs::write(output_path, &data)?;
    println!("Wrote {} to {}", data.len(), output_path.display());

    Ok(())
}

#[tokio::main]
async fn start_web_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting transmitwave server on http://localhost:{}", port);
    println!("Endpoints:");
    println!("  POST /encode - Encode binary data to WAV with multi-tone FSK (ggwave-compatible)");
    println!("  POST /decode - Decode WAV to binary data with FSK");
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

async fn handler_status() -> String {
    "transmitwave server with multi-tone FSK (ggwave-compatible) encoding/decoding - Ready".to_string()
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

    // Use FSK encoder (default for maximum reliability)
    let encode_result = EncoderFsk::new()
        .map_err(|e| e.to_string())
        .and_then(|mut encoder| {
            encoder.encode(&data)
                .map_err(|e| e.to_string())
        });

    match encode_result {
        Ok(samples) => {
            // Convert to WAV
            let spec = WavSpec {
                channels: 1,
                sample_rate: transmitwave_core::SAMPLE_RATE as u32,
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
                    "Encoded {} bytes to {} samples",
                    data.len(),
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

            // Use FSK decoder (default for maximum reliability)
            let decode_result = DecoderFsk::new()
                .map_err(|e| e.to_string())
                .and_then(|mut decoder| {
                    decoder.decode(&samples)
                        .map_err(|e| e.to_string())
                });

            match decode_result {
                Ok(decoded_data) => {
                    let data_base64 = base64::engine::general_purpose::STANDARD.encode(&decoded_data);
                    Ok(Json(DecodeResponse {
                        success: true,
                        message: format!(
                            "Decoded {} bytes",
                            decoded_data.len()
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

