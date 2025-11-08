# Audio Modem Library - FSK Mode

A Rust library for reliable low-bandwidth communication over audio channels using multi-tone FSK modulation. Encodes binary data into simultaneous audio frequencies (800-2700 Hz) for maximum robustness in speaker-to-microphone transmission scenarios.

## Demo with Fountain Code Mode


https://github.com/user-attachments/assets/6d3dce34-9152-4a55-a5dd-c0839e20063e



## Credit

This project is inspired by the [ggwave](https://github.com/ggerganov/ggwave) library by Georgi Gerganov. Both projects use a multi-tone FSK modulation scheme for data-over-sound transmission, but with different technical implementations:

**ggwave's approach:**
- Base frequency: 1875 Hz (audible mode)
- Frequency spacing: 46.875 Hz
- Sound markers for frame synchronization
- Default sample rate: 48 kHz

**transmitwave's approach:**
- Base frequency: 800 Hz (optimized for mobile phone speakers)
- Frequency spacing: 20 Hz (tighter spacing for more bins)
- Chirp-based preamble/postamble for synchronization
- Sample rate: 16 kHz

While both use similar multi-tone FSK principles (96 frequency bins, 6 tones per symbol, 3 bytes per transmission), the different parameters mean the protocols are **not directly compatible**. Transmitwave's lower base frequency and tighter spacing provide better performance on mobile device speakers, especially for iPhone and Android devices.

**Unique to transmitwave:**
- **Fountain Code Mode**: Supports RaptorQ fountain codes (RFC 6330) for rateless streaming transmission - ideal for unreliable channels and broadcast scenarios where continuous streaming is needed. Uses a distinctive three-note whistle preamble (800→1200→1600 Hz) instead of chirp for synchronization. See [FOUNTAIN_MODE.md](FOUNTAIN_MODE.md) for details.

## Try It Out

**Web Demo**: [transmitwave.andrewtheguy.com](https://transmitwave.andrewtheguy.com)

Experience transmitwave directly in your browser - **no backend server required**:
- **100% Frontend**: All encoding/decoding runs locally in your browser via WebAssembly
- **Standard Mode**: Encode/decode messages with Reed-Solomon error correction
- **Fountain Code Mode**: Continuous streaming with RaptorQ codes and manual start/stop
- **Cross-Device**: Test speaker-to-microphone transmission between devices

## Features

- **Multi-tone FSK Modulation**: 6 simultaneous audio frequencies for non-coherent energy detection
- **Mobile-Optimized Frequency Band**: Uses 800-2700 Hz for excellent mobile phone speaker compatibility
- **Reed-Solomon FEC**: Forward error correction for reliability
- **CRC Validation**: Integrity checks on frame headers
- **Preamble/Postamble Detection**: Frame synchronization for reliable reception
- **Fountain Code Mode**: RaptorQ rateless streaming for unreliable/broadcast channels (see [FOUNTAIN_MODE.md](FOUNTAIN_MODE.md))
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

## Status of Components

| Component | Status | Testing | Notes |
|-----------|--------|---------|-------|
| **Core Library** | ✅ Stable | Comprehensive unit tests | Robust implementation with extensive test coverage for FSK modulation, FEC, framing, and fountain codes |
| **CLI Tool** | ⚠️ Beta | Basic integration tests | Functional testing through CLI commands, less comprehensive than core tests |
| **WASM Library/Web Interface** | ⚠️ Beta | Manual testing only | Tested primarily through web interface, no automated unit tests yet |

## Configuration

**Audio Parameters:**
- Sample Rate: 16 kHz
- FSK Frequencies: 96 bins with 6 simultaneous tones (800-2700 Hz)
- Frequency Spacing: 20 Hz between adjacent bins
- Base Frequency: 800 Hz (optimized for mobile phone speakers)
- Symbol Duration: 192 ms (3072 samples per symbol) for robust low-frequency detection
- Preamble Duration: 250 ms for reliable synchronization
- Postamble Duration: 250 ms for end-of-frame detection
- Frequency Band: Optimized for mobile phone speaker reproduction

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
- **Frequency Range**: 800-2700 Hz (optimized for mobile phone speaker reproduction)

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
- Inspired by ggwave's multi-tone FSK approach but optimized for mobile phone speakers with lower frequencies (800 Hz base vs 1875 Hz)

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
- **Mobile-Optimized Band**: Uses 800-2700 Hz, optimized for mobile phone speaker reproduction (iPhone, Android)
- **Non-Coherent Detection**: Goertzel-based energy detection eliminates need for complex phase tracking
- **Over-the-Air**: Designed specifically for speaker-to-microphone audio transfer scenarios
- **96 Frequency Bins**: Provides sufficient redundancy and flexibility with 20 Hz spacing
