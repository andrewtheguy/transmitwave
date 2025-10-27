# Audio Modem Library - FSK Mode

A Rust library for reliable low-bandwidth communication over audio channels using multi-tone FSK modulation. Encodes binary data into simultaneous audio frequencies (400-2300 Hz) for maximum robustness in speaker-to-microphone transmission scenarios.

## Features

- **Multi-tone FSK Modulation**: 6 simultaneous audio frequencies for non-coherent energy detection
- **Sub-Bass Frequency Band**: Uses low frequencies (400-2300 Hz) for excellent room acoustics compatibility
- **Reed-Solomon FEC**: Forward error correction for reliability
- **CRC Validation**: Integrity checks on frame headers
- **Preamble/Postamble Detection**: Frame synchronization for reliable reception
- **Maximum Reliability**: Optimized for over-the-air audio transfer with minimal error rates
- **Multiple Targets**: Native CLI tool, WASM library, and core library

## Components

### Core Library (`core/`)
- `fsk.rs`: Multi-tone FSK modulation/demodulation
- `fec.rs`: Reed-Solomon error correction
- `framing.rs`: Frame structure with CRC
- `sync.rs`: Preamble/postamble generation and detection
- `encoder_fsk.rs`: Data-to-audio FSK encoding
- `decoder_fsk.rs`: Audio-to-data FSK decoding

### CLI Tool (`cli/`)
Native command-line tool for WAV file processing:

```bash
# Encode binary data to WAV audio using FSK
cargo run -- encode input.bin output.wav

# Decode WAV audio back to binary using FSK
cargo run -- decode input.wav output.bin
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
- FSK Frequencies: 96 bins with 6 simultaneous tones (400-2300 Hz)
- Frequency Spacing: 20 Hz between adjacent bins
- Symbol Duration: 192 ms (3072 samples per symbol) for robust low-frequency detection
- Preamble Duration: 250 ms for reliable synchronization
- Postamble Duration: 250 ms for end-of-frame detection
- Frequency Band: Sub-bass optimized for excellent room acoustics compatibility

**FEC Configuration:**
- Reed-Solomon: (255, 223) - 32 bytes ECC
- Frame Header: CRC-8 protection
- Max Payload: 200 bytes per frame

## Usage Examples

### Encoding Data with FSK

```rust
use transmitwave_core::EncoderFsk;

let mut encoder = EncoderFsk::new()?;
let data = b"Hello, World!";
let audio_samples = encoder.encode(data)?;
```

### Decoding Audio with FSK

```rust
use transmitwave_core::DecoderFsk;

let mut decoder = DecoderFsk::new()?;
let samples = load_audio_file("audio.wav")?;
let decoded_data = decoder.decode(&samples)?;
```

### CLI Example

```bash
# Create test data
echo "Test message" > test.bin

# Encode to audio
cargo run -- encode test.bin test.wav

# Decode back
cargo run -- decode test.wav decoded.bin

# Verify
diff test.bin decoded.bin
```

## Performance

- **Throughput**: ~16 bits/sec of actual data
- **Overhead**: Optimized for maximum reliability over speed
- **Latency**: ~2 seconds per 200-byte message
- **Frequency Range**: 400-2300 Hz (sub-bass band with excellent acoustic properties)

## Testing

Run all tests (release mode recommended for faster execution):
```bash
cargo test --workspace --release
```

The test suite includes:
- Unit tests for FEC, framing, and FSK components
- Integration tests for end-to-end encode/decode with various payload sizes and noise levels

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
[FSK Modulator] → Modulate bits onto 6 simultaneous tones
    ↓
[Preamble/Postamble] → Add sync signals
    ↓
Audio Samples (1600 samples per 100ms symbol)
    ↓ (transmission over audio channel)
    ↓
[Preamble Detector] → Find frame start
    ↓
[FSK Demodulator] → Extract bits from tone energies (Goertzel)
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
- 6 simultaneous FSK tones provide redundancy and robustness
- Non-coherent energy detection (Goertzel algorithm) eliminates phase synchronization burden
- Reed-Solomon FEC enables recovery from bit errors
- Preamble/postamble detection provides frame synchronization
- Very low bit rate (16 bps) allows use of simple, robust modulation
- ggwave-compatible frequency band ensures broad compatibility

## Dependencies

- **reed-solomon-erasure**: Forward error correction
- **hound**: WAV file I/O (CLI only)
- **wasm-bindgen**: JavaScript bindings (WASM only)
- **thiserror**: Error handling

## Building WASM

To build WASM for web use:

```bash
wasm-pack build wasm --release --target web
```

## Notes on FSK Mode

- **Reliability**: Multi-tone FSK with error correction provides robust transmission over typical audio channels
- **Low-Frequency Band**: Uses 400-2300 Hz (sub-bass) for excellent room acoustics and reduced reflections
- **Non-Coherent Detection**: Goertzel-based energy detection eliminates need for complex phase tracking
- **Over-the-Air**: Designed specifically for speaker-to-microphone audio transfer scenarios
- **96 Frequency Bins**: Provides sufficient redundancy and flexibility for adaptive frequency allocation
- **Legacy Modes Removed**: Legacy OFDM and spread spectrum modes have been removed to focus development on the most reliable FSK implementation
