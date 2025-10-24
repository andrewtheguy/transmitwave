# Audio Modem Library

A Rust library for reliable low-bandwidth communication over audio channels. Encodes binary data into OFDM signals within the audible range (0-4kHz), creating a distinctive "56k modem hiss" sound.

## Features

- **OFDM Modulation**: 224 overlapping subcarriers with phase randomization for white-noise-like hiss sound
- **Reed-Solomon FEC**: Forward error correction for reliability
- **CRC Validation**: Integrity checks on frame headers
- **Preamble/Postamble Detection**: Frame synchronization with chirp and tone signals
- **Very Low Throughput**: ~16 bits/sec actual data (by design for extreme reliability)
- **Multiple Targets**: Native CLI tool, WASM library, and core library

## Components

### Core Library (`core/`)
- `ofdm.rs`: OFDM modulation/demodulation
- `fec.rs`: Reed-Solomon error correction
- `framing.rs`: Frame structure with CRC
- `sync.rs`: Preamble/postamble generation and detection
- `encoder.rs`: Data-to-audio encoding
- `decoder.rs`: Audio-to-data decoding

### CLI Tool (`cli/`)
Native command-line tool for WAV file processing:

```bash
# Encode binary data to WAV audio
cargo run -p testaudio-cli --bin testaudio -- encode input.bin output.wav

# Decode WAV audio back to binary
cargo run -p testaudio-cli --bin testaudio -- decode input.wav output.bin
```

### WASM Library (`wasm/`)
JavaScript bindings for web applications:

```javascript
const encoder = new WasmEncoder();
const audioSamples = encoder.encode(dataArray);

const decoder = new WasmDecoder();
const recoveredData = decoder.decode(audioSamples);
```

## Configuration

**Audio Parameters:**
- Sample Rate: 16 kHz
- Subcarriers: 224 (400-3200 Hz) with deterministic phase randomization
- Symbol Duration: 100 ms
- Preamble: 300 ms chirp (200-4000 Hz)
- Postamble: 50 ms tone (2 kHz)
- Phase Randomization: Deterministic per-subcarrier phase offsets create white-noise-like hiss instead of tonal patterns

**FEC Configuration:**
- Reed-Solomon: (255, 223) - 32 bytes ECC
- Frame Header: CRC-8 protection
- Max Payload: 200 bytes per frame

## Usage Examples

### Encoding Data

```rust
use testaudio_core::Encoder;

let mut encoder = Encoder::new()?;
let data = b"Hello, World!";
let audio_samples = encoder.encode(data)?;
```

### Decoding Audio

```rust
use testaudio_core::Decoder;

let mut decoder = Decoder::new()?;
let samples = load_audio_file("audio.wav")?;
let decoded_data = decoder.decode(&samples)?;
```

### CLI Example

```bash
# Create test data
echo "Test message" > test.txt

# Encode to audio
cargo run -p testaudio-cli --bin testaudio -- encode test.txt test.wav

# Decode back
cargo run -p testaudio-cli --bin testaudio -- decode test.wav decoded.txt

# Verify
diff test.txt decoded.txt
```

## Performance

- **Throughput**: ~16 bits/sec of actual data
- **Overhead**: ~10x redundancy for reliability
- **Latency**: ~1 second per message
- **Frequency Range**: 200-4000 Hz (well within audible range)

## Testing

Run all tests:
```bash
cargo test --workspace
```

The test suite includes:
- Unit tests for each component (FEC, framing, OFDM, sync)
- Integration tests for end-to-end encode/decode

## Architecture

```
Input Data
    ↓
[Frame Encoder] → Add length, frame#, CRC
    ↓
[FEC Encoder] → Add Reed-Solomon ECC
    ↓
[Bit Converter] → Convert bytes to bits
    ↓
[OFDM Modulator] → Modulate bits onto subcarriers
    ↓
[Preamble/Postamble] → Add sync signals
    ↓
Audio Samples (1600 samples per 100ms symbol)
    ↓ (transmission over audio channel)
    ↓
[Preamble Detector] → Find frame start
    ↓
[OFDM Demodulator] → Extract bits from subcarriers
    ↓
[Bit Converter] → Reconstruct bytes
    ↓
[FEC Decoder] → Correct errors
    ↓
[Frame Decoder] → Verify CRC, extract payload
    ↓
Output Data
```

## Design Philosophy

This modem prioritizes **reliability** over throughput:
- Multiple overlapping frequencies reduce sensitivity to channel impairments
- Reed-Solomon FEC enables recovery from bit errors
- Preamble/postamble detection provides frame synchronization
- Very low bit rate (16 bps) allows use of simple, robust modulation

## Dependencies

- **rustfft**: FFT for OFDM
- **reed-solomon-erasure**: Forward error correction
- **hound**: WAV file I/O (CLI only)
- **wasm-bindgen**: JavaScript bindings (WASM only)
- **thiserror**: Error handling

## Building WASM

To build WASM for web use:

```bash
cargo build -p testaudio-wasm --target wasm32-unknown-unknown
wasm-pack build wasm --target web
```

## Future Improvements

- Implement full Reed-Solomon decoding with erasure correction
- Add noise robustness testing
- Support variable frame sizes
- Implement adaptive frequency allocation
- Add support for multi-frame messages
