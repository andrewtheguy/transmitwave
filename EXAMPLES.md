# Usage Examples

## Command-Line Tool

### Basic Encoding

```bash
# Create some test data
echo "Hello, World!" > tmp/message.txt

# Encode to WAV audio file
cargo run -p testaudio-cli --bin testaudio -- encode tmp/message.txt tmp/message.wav

# Output:
# Read 14 bytes from message.txt
# Encoded to 74400 audio samples
# Wrote message.wav to 1
```

### Decoding Back

```bash
# Decode the WAV file
cargo run -p testaudio-cli --bin testaudio -- decode tmp/message.wav tmp/recovered.txt

# Output:
# Read WAV: 16000 Hz, 1 channels, 32 bits
# Extracted 74400 samples
# Decoded 14 bytes
# Wrote 14 to recovered.txt

# Verify it matches
diff message.txt recovered.txt
# (no output = files are identical)
```

### Working with Binary Files

```bash
# Create random binary data
dd if=/dev/urandom of=binary_data.bin bs=1 count=100

# Encode
cargo run -p testaudio-cli --bin testaudio -- encode binary_data.bin binary.wav

# Decode
cargo run -p testaudio-cli --bin testaudio -- decode binary.wav binary_recovered.bin

# Verify
cmp binary_data.bin binary_recovered.bin && echo "Perfect match!"
```

### Large Files (up to 200 bytes)

```bash
# Create a 200-byte file (maximum payload size)
head -c 200 /dev/urandom > large.bin

# Encode (takes about 1 second to generate audio)
time cargo run -p testaudio-cli --bin testaudio -- encode large.bin large.wav

# Decode
time cargo run -p testaudio-cli --bin testaudio -- decode large.wav large_recovered.bin

# Verify
cmp large.bin large_recovered.bin && echo "Success!"
```

## Frequency-Hopping Spread Spectrum (FHSS)

### Overview

FHSS improves resistance to narrowband interference by hopping the signal across multiple frequency bands:
- **1 band** (default): No hopping, backward compatible
- **2 bands**: Basic hopping across 400-3200 Hz split
- **3 bands**: Recommended for good interference resistance
- **4 bands**: Maximum hopping for harsh environments

### Command-Line Examples

#### Default (No FHSS)
```bash
# Backward compatible, single band (400-3200 Hz)
testaudio encode data.bin audio.wav
testaudio decode audio.wav data.bin
```

#### 2-Band FHSS
```bash
# Each symbol hops between 2 bands
testaudio encode data.bin audio_2band.wav --num-hops 2
testaudio decode audio_2band.wav data.bin --num-hops 2
```

#### 3-Band FHSS (Recommended)
```bash
# Best balance of interference resistance and overhead
testaudio encode data.bin audio_3band.wav --num-hops 3
testaudio decode audio_3band.wav data.bin --num-hops 3
```

#### 4-Band FHSS (Maximum)
```bash
# Maximum frequency diversity for severe interference
testaudio encode data.bin audio_4band.wav --num-hops 4
testaudio decode audio_4band.wav data.bin --num-hops 4
```

#### Combined with Custom Chip Duration
```bash
# Combine FHSS with custom spreading
testaudio encode data.bin audio.wav --chip-duration 3 --num-hops 3
testaudio decode audio.wav data.bin --chip-duration 3 --num-hops 3
```

### FHSS Frequency Bands

For 3-band FHSS (most common):
- **Band 0:** 400-1333 Hz
- **Band 1:** 1333-2267 Hz
- **Band 2:** 2267-3200 Hz

The hopping pattern is deterministic (LFSR-based pseudorandom), so encoder and decoder automatically synchronize.

### Important: Encoder/Decoder Must Match

⚠️ **CRITICAL:** The `--num-hops` value MUST be the same for both encoding and decoding:

```bash
# ✅ Correct: matching num-hops
testaudio encode data.bin audio.wav --num-hops 3
testaudio decode audio.wav data.bin --num-hops 3

# ❌ Wrong: mismatched num-hops (will produce garbage or fail)
testaudio encode data.bin audio.wav --num-hops 3
testaudio decode audio.wav data.bin --num-hops 2  # Different = fails!
```

### Real-World Workflow Example

```bash
# 1. Create test data
echo "Important message with FHSS protection" > message.txt

# 2. Convert to binary
od -An -tx1 message.txt | tr -d ' \n' | xxd -r -p > message.bin

# 3. Encode with 3-band FHSS (best for interference resistance)
testaudio encode message.bin message_secure.wav --num-hops 3

# 4. Transmit message_secure.wav over audio channel
# (more resistant to narrowband interference than default mode)

# 5. Decode with matching settings
testaudio decode message_secure.wav decoded_message.bin --num-hops 3

# 6. Verify message integrity
od -An -tx1 decoded_message.bin | tr -d ' \n' | xxd -r -p
# Should output: Important message with FHSS protection
```

### When to Use Each Mode

| Mode | Bands | Use Case |
|------|-------|----------|
| Default | 1 | Clean environments, backward compatibility |
| 2-Band | 2 | Light narrowband interference |
| 3-Band | 3 | **Recommended** for most real-world scenarios |
| 4-Band | 4 | Severe interference, harsh conditions |

## Rust Library Usage

### Basic Encoding (Without FHSS)

```rust
use testaudio_core::Encoder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = b"Hello, Audio!";

    let mut encoder = Encoder::new()?;
    let samples = encoder.encode(data)?;

    println!("Generated {} audio samples", samples.len());

    Ok(())
}
```

### Encoding with FHSS

```rust
use testaudio_core::EncoderSpread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = b"FHSS protected message";

    // Create encoder with 3-band FHSS
    let mut encoder = EncoderSpread::with_fhss(
        2,  // chip_duration (samples per Barker chip)
        3   // num_frequency_hops (2-4 for FHSS, 1 to disable)
    )?;

    let samples = encoder.encode(data)?;
    println!("Encoded with FHSS: {} samples", samples.len());

    Ok(())
}
```

### Decoding with FHSS

```rust
use testaudio_core::DecoderSpread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let samples = vec![/* audio samples */];

    // Decoder MUST use same num_frequency_hops as encoder!
    let mut decoder = DecoderSpread::with_fhss(
        2,  // chip_duration (must match encoder)
        3   // num_frequency_hops (must match encoder)
    )?;

    let data = decoder.decode(&samples)?;
    println!("Recovered: {:?}", String::from_utf8_lossy(&data));

    Ok(())
}
```

### Complete Round-Trip Example with FHSS

```rust
use testaudio_core::{EncoderSpread, DecoderSpread};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let original_data = b"Test FHSS message";
    let num_hops = 3;  // 3-band FHSS
    let chip_duration = 2;

    // Encode
    let mut encoder = EncoderSpread::with_fhss(chip_duration, num_hops)?;
    let samples = encoder.encode(original_data)?;
    println!("Encoded {} bytes to {} samples", original_data.len(), samples.len());

    // Decode (with matching settings)
    let mut decoder = DecoderSpread::with_fhss(chip_duration, num_hops)?;
    let recovered_data = decoder.decode(&samples)?;
    println!("Decoded: {:?}", String::from_utf8_lossy(&recovered_data));

    // Verify
    assert_eq!(original_data, &recovered_data[..]);
    println!("✅ Round-trip successful!");

    Ok(())
}
```

### Basic Decoding

```rust
use testaudio_core::Decoder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let samples = vec![/* audio samples */];

    let mut decoder = Decoder::new()?;
    let data = decoder.decode(&samples)?;

    println!("Recovered: {:?}", String::from_utf8_lossy(&data));

    Ok(())
}
```

### Working with WAV Files

```rust
use testaudio_core::Encoder;
use hound::WavSpec;
use std::fs::File;

fn encode_to_wav(data: &[u8], wav_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut encoder = Encoder::new()?;
    let samples = encoder.encode(data)?;

    // Write WAV file
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let file = File::create(wav_path)?;
    let mut writer = hound::WavWriter::new(file, spec)?;

    for sample in samples {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;

    Ok(())
}
```

```rust
use testaudio_core::Decoder;
use std::fs::File;

fn decode_from_wav(wav_path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let file = File::open(wav_path)?;
    let mut reader = hound::WavReader::new(file)?;

    // Extract float samples
    let samples: Result<Vec<f32>, _> = reader.samples::<f32>().collect();
    let samples = samples?;

    // Decode
    let mut decoder = Decoder::new()?;
    let data = decoder.decode(&samples)?;

    Ok(data)
}
```

### Error Handling

```rust
use testaudio_core::{Encoder, Decoder, AudioModemError};

fn main() {
    let data = b"Test";
    let mut encoder = Encoder::new().expect("Failed to initialize encoder");

    match encoder.encode(data) {
        Ok(samples) => println!("Success: {} samples", samples.len()),
        Err(AudioModemError::InvalidInputSize) => {
            eprintln!("Data is too large (max 200 bytes)");
        }
        Err(e) => eprintln!("Encoding failed: {}", e),
    }
}
```

## JavaScript/WASM Usage

### Building WASM Module

```bash
# Install wasm-pack if not already installed
cargo install wasm-pack

# Build for web
wasm-pack build wasm --target web

# This generates:
# - pkg/testaudio_wasm.js (JavaScript wrapper)
# - pkg/testaudio_wasm_bg.wasm (WebAssembly binary)
```

### Using in HTML/JavaScript

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8" />
    <title>Audio Modem Example</title>
</head>
<body>
    <h1>Audio Modem Test</h1>

    <input type="text" id="messageInput" placeholder="Enter message" />
    <button onclick="encodeMessage()">Encode to Audio</button>
    <button onclick="playAudio()">Play Audio</button>
    <button onclick="decodeAudio()">Decode Audio</button>

    <p>Decoded: <span id="result"></span></p>

    <script type="module">
        import init, { WasmEncoder, WasmDecoder }
            from './pkg/testaudio_wasm.js';

        let audioBuffer = null;
        let audioContext = null;

        async function initialize() {
            await init();
            audioContext = new (window.AudioContext || window.webkitAudioContext)();
        }

        window.encodeMessage = function() {
            const message = document.getElementById('messageInput').value;
            const encoder = new WasmEncoder();

            // Convert string to Uint8Array
            const data = new TextEncoder().encode(message);

            try {
                const samples = encoder.encode(data);
                audioBuffer = samples;
                console.log(`Encoded ${message.length} bytes to ${samples.length} audio samples`);
                document.getElementById('result').textContent = `Encoded: ${samples.length} samples`;
            } catch (e) {
                console.error('Encoding failed:', e);
            }
        };

        window.playAudio = function() {
            if (!audioBuffer) {
                alert('No audio data. Encode first!');
                return;
            }

            const audioBuffer2 = audioContext.createBuffer(
                1, audioBuffer.length, 16000
            );
            const channel = audioBuffer2.getChannelData(0);
            channel.set(audioBuffer);

            const source = audioContext.createBufferSource();
            source.buffer = audioBuffer2;
            source.connect(audioContext.destination);
            source.start();
        };

        window.decodeAudio = function() {
            if (!audioBuffer) {
                alert('No audio data. Encode and play first!');
                return;
            }

            const decoder = new WasmDecoder();

            try {
                const data = decoder.decode(audioBuffer);
                const message = new TextDecoder().decode(data);
                document.getElementById('result').textContent = `Decoded: ${message}`;
                console.log('Decoded message:', message);
            } catch (e) {
                console.error('Decoding failed:', e);
                document.getElementById('result').textContent = `Error: ${e}`;
            }
        };

        // Initialize on page load
        window.addEventListener('load', initialize);
    </script>
</body>
</html>
```

## Audio Format Information

### Generated Audio Characteristics

- **Sample Rate:** 16 kHz (compatible with 44.1 kHz and 48 kHz systems)
- **Bit Depth:** 32-bit float
- **Channels:** Mono
- **Frequency Range:** 200-4000 Hz (well within audible range)
- **Sound Character:** Dense hissing tone (OFDM carrier aggregation)

### File Sizes

- **Fixed preamble:** 4,800 samples (0.3 seconds) = ~19 KB
- **Data portion:** ~1,600 samples per 48 bits (~100 bytes FEC-encoded)
  - 22 bytes → 255 bytes FEC → ~1,600 samples → ~6.4 KB
- **Fixed postamble:** 800 samples (0.05 seconds) = ~3.2 KB
- **Total for 22-byte message:** ~74 KB WAV file

For a 200-byte message:
- Data: ~1,600 bytes FEC-encoded → ~6,400 samples → ~25.6 KB
- Total: ~48 KB WAV file

## Testing

### Run All Tests

```bash
# Unit and integration tests
cargo test --workspace

# With output
cargo test --workspace -- --nocapture

# Specific test
cargo test test_encode_decode_round_trip -- --nocapture
```

### Manual Round-Trip Test

```bash
#!/bin/bash
# Create test file
echo "Test message $(date)" > test.txt

# Encode
cargo run -p testaudio-cli --bin testaudio -- encode test.txt test.wav

# Decode
cargo run -p testaudio-cli --bin testaudio -- decode test.wav decoded.txt

# Verify
if diff test.txt decoded.txt > /dev/null; then
    echo "✅ Round-trip successful!"
else
    echo "❌ Round-trip failed!"
fi
```

## Performance Considerations

### Encoding Time
- ~1 second per message (FFT operations)
- Scales with message size
- Can be optimized with release builds: `--release`

### Decoding Time
- ~0.5-1 second per message
- Preamble detection is the main cost
- Can be optimized with release builds

### Audio Playback
- Messages are designed for direct audio playback
- Can be processed in real-time by streaming decoders
- Works with any standard audio hardware

## Troubleshooting

### "PreambleNotFound" Error
- The decoder couldn't find the synchronization preamble
- Ensure audio wasn't corrupted or truncated
- Check that sample rate is correct (16 kHz expected)

### "PostambleNotFound" Error
- End-of-frame marker wasn't detected
- Audio might be cut off early
- Ensure full audio file is being decoded

### "CrcMismatch" Error
- Frame header corruption detected
- Indicates data loss during transmission
- FEC can potentially recover this

### "InvalidFrameSize" Error
- Decoded data doesn't match expected frame format
- Audio might have been severely corrupted
- Try with a shorter message

## Tips for Success

1. **Use Binary Data:** Text is more fragile; binary allows any byte value
2. **Keep Messages Small:** Start with < 50 bytes for testing
3. **High-Quality Audio:** Avoid heavy compression (AAC, MP3)
4. **Direct Transmission:** WAV format is ideal; minimize re-encoding
5. **Release Mode:** Use `--release` for better performance
6. **Test First:** Always test with text before binary data
