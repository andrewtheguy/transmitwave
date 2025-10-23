use clap::{Parser, Subcommand};
use hound::WavSpec;
use std::fs::File;
use std::path::PathBuf;
use testaudio_core::{Decoder, Encoder, DecoderSpread, EncoderSpread};

#[derive(Parser)]
#[command(name = "testaudio")]
#[command(about = "Audio modem for reliable low-bandwidth communication")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Encode binary data to WAV audio file
    Encode {
        /// Input binary file
        #[arg(value_name = "INPUT.BIN")]
        input: PathBuf,

        /// Output WAV file
        #[arg(value_name = "OUTPUT.WAV")]
        output: PathBuf,
    },

    /// Decode WAV audio file to binary data
    Decode {
        /// Input WAV file
        #[arg(value_name = "INPUT.WAV")]
        input: PathBuf,

        /// Output binary file
        #[arg(value_name = "OUTPUT.BIN")]
        output: PathBuf,
    },

    /// Encode with spread spectrum (Barker code redundancy)
    EncodeSpread {
        /// Input binary file
        #[arg(value_name = "INPUT.BIN")]
        input: PathBuf,

        /// Output WAV file
        #[arg(value_name = "OUTPUT.WAV")]
        output: PathBuf,

        /// Chip duration (samples per Barker chip, default: 2)
        #[arg(short, long, default_value = "2")]
        chip_duration: usize,
    },

    /// Decode spread spectrum WAV file to binary data
    DecodeSpread {
        /// Input WAV file
        #[arg(value_name = "INPUT.WAV")]
        input: PathBuf,

        /// Output binary file
        #[arg(value_name = "OUTPUT.BIN")]
        output: PathBuf,

        /// Chip duration (samples per Barker chip, must match encoder)
        #[arg(short, long, default_value = "2")]
        chip_duration: usize,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Encode { input, output } => encode_command(&input, &output)?,
        Commands::Decode { input, output } => decode_command(&input, &output)?,
        Commands::EncodeSpread { input, output, chip_duration } => {
            encode_spread_command(&input, &output, chip_duration)?
        }
        Commands::DecodeSpread { input, output, chip_duration } => {
            decode_spread_command(&input, &output, chip_duration)?
        }
    }

    Ok(())
}

fn encode_command(input_path: &PathBuf, output_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Read input binary file
    let data = std::fs::read(input_path)?;
    println!("Read {} bytes from {}", data.len(), input_path.display());

    // Create encoder and encode data
    let mut encoder = Encoder::new()?;
    let samples = encoder.encode(&data)?;
    println!("Encoded to {} audio samples", samples.len());

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

fn decode_command(input_path: &PathBuf, output_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Read WAV file
    let file = File::open(input_path)?;
    let mut reader = hound::WavReader::new(file)?;

    let spec = reader.spec();
    println!(
        "Read WAV: {} Hz, {} channels, {} bits",
        spec.sample_rate, spec.channels, spec.bits_per_sample
    );

    // Extract samples (handle both 16-bit and 32-bit float formats)
    let samples = match spec.bits_per_sample {
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
    let samples = match spec.bits_per_sample {
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

    // Decode with spread spectrum
    let mut decoder = DecoderSpread::new(chip_duration)?;
    let data = decoder.decode(&samples)?;
    println!("Decoded {} bytes (chip_duration={})", data.len(), chip_duration);

    // Write binary file
    std::fs::write(output_path, &data)?;
    println!("Wrote {} to {}", data.len(), output_path.display());

    Ok(())
}
