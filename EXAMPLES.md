# Usage Examples

## Command-Line Tool

### Basic Encoding

```bash
# Create some test data
echo "Hello, World!" > message.txt

# Encode to WAV audio file
cargo run -p testaudio-cli --bin testaudio -- encode message.txt message.wav

# Output:
# Read 14 bytes from message.txt
# Encoded to 74400 audio samples
# Wrote message.wav to 1
```

### Decoding Back

```bash
# Decode the WAV file
cargo run -p testaudio-cli --bin testaudio -- decode message.wav recovered.txt

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

## Rust Library Usage

### Basic Encoding

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
