# Fountain Code Mode

## Overview

Fountain code mode is a specialized transmission mode designed for unreliable or one-way communication channels where the sender continuously streams data until the receiver successfully decodes it. It uses **RaptorQ fountain codes (RFC 6330)** for robust erasure coding.

## What are Fountain Codes?

Fountain codes are a class of erasure codes that can generate a potentially infinite stream of encoding packets from a source message. Key properties:

- **Rateless**: Can generate as many encoding packets as needed
- **Erasure resilient**: Receiver can decode from any sufficient subset of packets
- **No feedback required**: Sender doesn't need acknowledgments from receiver

### RaptorQ vs LT Codes

This implementation uses **RaptorQ codes (RFC 6330)**, not simple Luby Transform (LT) codes:

- **Architecture**: Two-layer concatenated code (outer pre-code + inner LT code)
- **Performance**: Vastly superior decoding probability and efficiency compared to LT codes alone
- **Standards**: IETF RFC 6330 - the most advanced fountain code specification
- **Advantages**: Better support for larger blocks, lower overhead, more reliable decoding

## Design Principles

### Open-Ended Streaming
- Uses **only preamble signaling** (no postamble)
- Blocks can be transmitted continuously without boundaries
- Receiver can start listening at any point in the stream
- Ideal for broadcast scenarios or unreliable channels

### Audio Duration Control
The `timeout_secs` parameter controls the **total audio duration**, not CPU time:
- 30 second timeout → generates ~30 seconds of audio
- Prevents generating excessive data (hours of audio for small payloads)
- Ensures practical file sizes and transmission times

### Block Structure
Each fountain block contains:
```
[Preamble] [Frame Metadata] [Packet Length] [RaptorQ Packet] [Padding]
```

- **Preamble**: Synchronization signal for block detection
- **Frame Metadata**: 6 bytes (4 for frame_length, 2 for symbol_size)
- **Packet Length**: 2 bytes (enables padding removal)
- **RaptorQ Packet**: Serialized encoding packet (source or repair)
- **Padding**: Aligns to FSK symbol boundaries

## Configuration

### FountainConfig Parameters

```rust
pub struct FountainConfig {
    pub timeout_secs: u32,        // Audio duration in seconds (default: 30)
    pub block_size: usize,         // Symbol size in bytes (default: 64)
    pub repair_blocks_ratio: f32,  // Repair overhead ratio (default: 0.5)
}
```

#### timeout_secs
- Controls total **audio stream duration**
- Example: 30 seconds generates ~6 blocks for 21 bytes of data
- Longer timeouts provide more redundancy but larger files

#### block_size
- Size of each RaptorQ symbol in bytes
- Smaller = more blocks, better granularity
- Larger = fewer blocks, faster processing
- Must match between encoder and decoder

#### repair_blocks_ratio
- Ratio of repair packets to source packets
- 0.5 = 50% overhead (recommended)
- Higher values = more redundancy but longer streams
- 0.0 = no repair packets (source packets only)

## Usage

### CLI Commands

#### Encoding
```bash
transmitwave fountain-encode input.txt output.wav --timeout 30 --block-size 64 --repair-ratio 0.5
```

Parameters:
- `--timeout`: Audio duration in seconds (default: 30)
- `--block-size`: RaptorQ symbol size (default: 64)
- `--repair-ratio`: Repair packet overhead (default: 0.5)

#### Decoding
```bash
transmitwave fountain-decode input.wav output.txt --timeout 30 --block-size 64
```

Parameters:
- `--timeout`: Maximum time to spend decoding (default: 30)
- `--block-size`: Must match encoder's block size (default: 64)

### API Usage

#### Encoding
```rust
use transmitwave_core::{EncoderFsk, FountainConfig};

let mut encoder = EncoderFsk::new()?;
let data = b"Hello fountain mode!";

let config = FountainConfig {
    timeout_secs: 30,
    block_size: 64,
    repair_blocks_ratio: 0.5,
};

// Create fountain stream
let stream = encoder.encode_fountain(data, Some(config))?;

// Collect blocks (iterator stops after timeout_secs of audio)
let blocks: Vec<Vec<f32>> = stream.collect();

// Concatenate all audio samples
let audio_samples: Vec<f32> = blocks.into_iter().flatten().collect();
```

#### Decoding
```rust
use transmitwave_core::{DecoderFsk, FountainConfig};

let mut decoder = DecoderFsk::new()?;

let config = FountainConfig {
    timeout_secs: 30,
    block_size: 64,
    repair_blocks_ratio: 0.5,
};

// Decode from audio samples
let decoded_data = decoder.decode_fountain(&audio_samples, Some(config))?;
```

## How It Works

### Encoding Process

1. **Frame Creation**: Input data is wrapped in a frame with CRC and header
2. **RaptorQ Encoding**: Frame is encoded using RaptorQ with configured symbol size
3. **Packet Generation**: Iterator generates packets in this order:
   - All source packets (original data blocks)
   - Repair packets (generated for redundancy)
   - Cycle repeats until audio duration limit reached
4. **Audio Modulation**: Each packet becomes an audio block with preamble + FSK data

### Decoding Process

1. **Preamble Detection**: Scans audio for synchronization signals
2. **Block Extraction**: Demodulates FSK data following each preamble
3. **Metadata Parsing**: Extracts frame length and symbol size from first block
4. **RaptorQ Decoding**: Feeds packets to RaptorQ decoder
5. **Success**: Returns decoded data when sufficient packets received
6. **Timeout**: Fails if unable to decode within configured timeout

### Redundancy and Recovery

The fountain code can recover from:
- **Packet loss**: Receiver needs any N packets to decode N-packet source
- **Late start**: Receiver can start listening mid-stream
- **Poor SNR**: Repair packets provide redundancy for corrupted blocks

Example: For 21 bytes of data with 50% repair ratio:
- Source packets: ~4 blocks
- Repair packets: ~2 blocks
- Total: ~6 blocks over 30 seconds
- Can decode from any 4+ valid packets

## Performance Characteristics

### Audio Duration

For typical small payloads (< 100 bytes):
- Each block: ~80,000 samples (~5 seconds at 16kHz)
- 30 second timeout: ~6 blocks
- File size: ~1MB WAV file for 30 seconds

### Decoding Requirements

Minimum packets needed to decode:
```
min_packets = ceil(data_size / symbol_size)
```

With redundancy, decoder typically succeeds after receiving:
```
packets_needed ≈ min_packets × (1 + packet_loss_rate)
```

### Trade-offs

| Parameter | Increase → | Benefit | Cost |
|-----------|-----------|---------|------|
| timeout_secs | Longer audio | More redundancy | Larger files |
| block_size | Larger symbols | Fewer blocks | Less granular recovery |
| repair_ratio | More repair packets | Better reliability | Longer transmission |

## Comparison with Standard Mode

| Feature | Standard Mode | Fountain Mode |
|---------|--------------|---------------|
| FEC | Reed-Solomon | RaptorQ (RFC 6330) |
| Structure | Single frame with postamble | Continuous blocks, preamble-only |
| Use case | Point-to-point reliable link | Broadcast or unreliable channel |
| Feedback | None | None |
| Redundancy | Fixed (FEC overhead) | Configurable (repair ratio) |
| Recovery | Error correction within frame | Any sufficient subset of packets |

## Example Scenarios

### Scenario 1: Reliable Short Message
```bash
# Small data, low redundancy, quick transmission
transmitwave fountain-encode message.txt output.wav --timeout 10 --repair-ratio 0.3
```

### Scenario 2: Critical Data with High Loss
```bash
# Important data, high redundancy, longer transmission
transmitwave fountain-encode critical.dat output.wav --timeout 60 --repair-ratio 1.0
```

### Scenario 3: Broadcast to Multiple Receivers
```bash
# Continuous transmission until all receivers decode
transmitwave fountain-encode broadcast.txt output.wav --timeout 120 --repair-ratio 0.5
```

## Limitations

- **Not for real-time**: Encoding generates full stream upfront
- **Memory usage**: Entire audio stream held in memory before writing
- **File size**: Longer timeouts create large WAV files
- **Block size constraint**: Must be consistent between encoder/decoder
- **No acknowledgment**: Sender doesn't know when receiver succeeds

## Testing

Run fountain mode tests:
```bash
cargo test -p transmitwave-core fountain -- --nocapture
```

Tests cover:
- Basic encoding/decoding roundtrip
- Packet loss scenarios (33% loss)
- Various data sizes (1 byte to 100 bytes)
- Timeout behavior
- Configuration validation

## Technical References

- **RFC 6330**: RaptorQ Forward Error Correction Scheme
- **RaptorQ Rust crate**: https://crates.io/crates/raptorq
- **Fountain Codes**: Luby, M. (2002) - "LT codes" foundation
- **Raptor Codes**: Shokrollahi, A. (2006) - "Raptor codes" improvement

## Future Improvements

Potential enhancements:
- Streaming encoding (generate blocks on-demand)
- Dynamic repair ratio based on channel conditions
- Metadata-only mode for very small payloads
- Interleaving for burst error resilience
- Progressive decoding feedback
