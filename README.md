# Audio Modem Library

A Rust library for reliable low-bandwidth communication over audio channels. Encodes binary data into OFDM signals within the audible range (0-4kHz), creating a distinctive "56k modem hiss" sound.

## Features

- **OFDM Modulation**: 48 overlapping subcarriers for robust multi-frequency transmission
- **Frequency-Hopping Spread Spectrum (FHSS)**: NEW! Optional multi-band hopping for improved interference resistance
- **Reed-Solomon FEC**: Forward error correction for reliability
- **CRC Validation**: Integrity checks on frame headers
- **Preamble/Postamble Detection**: Frame synchronization with chirp and tone signals
- **Very Low Throughput**: ~16 bits/sec actual data (by design for extreme reliability)
- **Multiple Targets**: Native CLI tool, WASM library, and core library

## Components

### Core Library (`core/`)
- `fhss.rs`: **NEW!** Frequency-hopping spread spectrum (LFSR-based hopping patterns)
- `ofdm.rs`: OFDM modulation/demodulation with FHSS support
- `fec.rs`: Reed-Solomon error correction
- `framing.rs`: Frame structure with CRC
- `sync.rs`: Preamble/postamble generation and detection
- `encoder.rs`: Data-to-audio encoding
- `decoder.rs`: Audio-to-data decoding
- `encoder_spread.rs`: Barker spreading with FHSS support
- `decoder_spread.rs`: Barker despreading with FHSS support

### CLI Tool (`cli/`)
Native command-line tool for WAV file processing with FHSS support:

```bash
# Encode binary data to WAV audio (default: no FHSS)
cargo run -p testaudio-cli --bin testaudio -- encode input.bin output.wav

# Decode WAV audio back to binary (default: no FHSS)
cargo run -p testaudio-cli --bin testaudio -- decode input.wav output.bin

# With FHSS (3-band, recommended for interference resistance)
cargo run -p testaudio-cli --bin testaudio -- encode input.bin output.wav --num-hops 3
cargo run -p testaudio-cli --bin testaudio -- decode input.wav output.bin --num-hops 3
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
- Subcarriers: 48 (200-4000 Hz)
- Symbol Duration: 100 ms
- Preamble: 300 ms chirp (200-4000 Hz)
- Postamble: 50 ms tone (2 kHz)

**FEC Configuration:**
- Reed-Solomon: (255, 223) - 32 bytes ECC
- Frame Header: CRC-8 protection
- Max Payload: 200 bytes per frame

**FHSS Configuration:**
- Number of Bands: 1-4 (default: 1 = no hopping)
- Hopping Pattern: LFSR-based pseudorandom (deterministic)
- Band Range: 400-3200 Hz distributed across selected bands

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

### FHSS Example (NEW)

```bash
# Create test data
echo "FHSS protected message" > secure.txt

# Encode with 3-band FHSS for interference resistance
cargo run -p testaudio-cli --bin testaudio -- encode secure.txt secure.wav --num-hops 3

# Decode with matching FHSS parameters (MUST match!)
cargo run -p testaudio-cli --bin testaudio -- decode secure.wav decoded.txt --num-hops 3

# Verify integrity
diff secure.txt decoded.txt
```

**FHSS Benefits:**
- 🛡️ Improved resistance to narrowband interference/jamming
- 📡 Better performance in frequency-selective fading channels
- 🔄 Frequency diversity improves overall reliability
- ⚡ Zero additional latency or overhead
- ↩️ Backward compatible (default 1-band = original behavior)

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
