use clap::{Parser, Subcommand};
use hound::WavSpec;
use std::fs::File;
use std::path::PathBuf;
use testaudio_core::{Decoder, Encoder};

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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Encode { input, output } => encode_command(&input, &output)?,
        Commands::Decode { input, output } => decode_command(&input, &output)?,
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

    // Write WAV file
    let spec = WavSpec {
        channels: 1,
        sample_rate: testaudio_core::SAMPLE_RATE as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let file = File::create(output_path)?;
    let mut writer = hound::WavWriter::new(file, spec)?;

    for sample in samples {
        writer.write_sample(sample)?;
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

    // Extract samples
    let samples: Result<Vec<f32>, _> = reader.samples::<f32>().collect();
    let samples = samples?;
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
