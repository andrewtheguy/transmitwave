# Chirp Spread Spectrum (CSS) Modulation

## Overview

CSS (Chirp Spread Spectrum) is an alternative to OFDM that produces a smooth "hiss" sound throughout the entire transmission, similar to the preamble/postamble chirps and classic 56k modems. Instead of using discrete frequency bins that create distinct tones, CSS encodes data as continuous chirp signals that sweep smoothly across the frequency spectrum.

## How It Works

Each data bit is encoded as a chirp signal:
- **Bit 1**: Up-chirp (200 Hz → 4000 Hz)
- **Bit 0**: Down-chirp (4000 Hz → 200 Hz)

Each chirp lasts 50ms (800 samples at 16kHz), creating a continuous frequency modulation with no discrete tones.

**Transmission Structure:**
```
[Preamble Chirp] [CSS Data Bits] [Postamble Chirp]
  (250ms hiss)   (series of chirps)  (250ms hiss)
```

## Audio Characteristics

- **Sound**: Continuous hiss/whistle similar to preamble/postamble
- **Frequency range**: 200-4000 Hz (same as OFDM)
- **Symbol duration**: 50ms per bit (smooth transitions)
- **Bit rate**: ~20 bits/second (before FEC)
- **Robustness**: Excellent resistance to noise and multipath

## Usage Examples

### CLI

Encode with CSS:
```bash
testaudio encode input.bin output.wav --css
```

Decode CSS:
```bash
testaudio decode input.wav output.bin --css
```

### Rust API

```rust
use testaudio_core::{EncoderCss, DecoderCss};

// Encode
let mut encoder = EncoderCss::new()?;
let samples = encoder.encode(b"Hello")?;

// Decode
let mut decoder = DecoderCss::new()?;
let data = decoder.decode(&samples)?;
```

### WASM/Web

```javascript
import { WasmEncoderCss, WasmDecoderCss } from 'testaudio-wasm';

// Encode
const encoder = new WasmEncoderCss();
const samples = encoder.encode(data);

// Decode
const decoder = new WasmDecoderCss();
const decoded = decoder.decode(samples);
```

Using factory functions:
```javascript
import { createEncoder, createDecoder } from '@/utils/wasm';

const encoder = await createEncoder({ type: 'css' });
const decoder = await createDecoder({ type: 'css' });
```

## Comparison with OFDM

| Feature | OFDM | CSS |
|---------|------|-----|
| Sound | Distinct tones ("cranking") | Smooth hiss |
| Bit rate | ~48 bits/symbol | ~1 bit/symbol |
| Symbol duration | 100ms | 50ms |
| Complexity | Higher (FFT) | Lower (correlation) |
| Robustness | Good | Excellent |
| Frequency use | 48 discrete bins | Continuous sweep |
| Aesthetic | Modem-like with tones | Continuous whistle |

## When to Use CSS

Choose CSS modulation when you want:
- The entire transmission to sound like a "hiss" (similar to Quiet library or 56k modems)
- Maximum robustness over throughput
- To avoid discrete tonal components
- An application where the "modem sound" aesthetic is important

Choose OFDM when you want:
- Higher data throughput (~48 bits per symbol)
- Faster transmission times
- More efficient spectrum usage

## Technical Details

### Configuration Constants

```rust
pub const CSS_SYMBOL_DURATION_MS: usize = 50;      // 50ms per bit
pub const CSS_SAMPLES_PER_SYMBOL: usize = 800;     // At 16kHz sample rate
pub const CSS_START_FREQ: f32 = 200.0;             // Hz
pub const CSS_END_FREQ: f32 = 4000.0;              // Hz
```

### Modulation Process

1. **Bit to Chirp Conversion**: Each bit is converted to a chirp template
   - Up-chirp for bit=1
   - Down-chirp for bit=0

2. **Frame Structure**: Data is wrapped in preamble/postamble for synchronization
   - Preamble: 250ms ascending chirp (200→4000 Hz)
   - Data: Variable duration CSS-modulated bits
   - Postamble: 250ms descending chirp (4000→200 Hz)

3. **FEC & Framing**: Standard Reed-Solomon (255,223) FEC is applied before CSS modulation

### Demodulation Process

1. **Preamble Detection**: Correlate with ascending chirp template
2. **Data Extraction**: Extract samples between preamble and postamble
3. **Symbol Demodulation**: For each 50ms symbol:
   - Correlate with up-chirp template
   - Correlate with down-chirp template
   - Decide bit value based on stronger correlation
4. **FEC Decoding**: Apply Reed-Solomon decoding
5. **Frame Decoding**: Extract payload from frame

### FFT-Based Correlation

CSS demodulation uses FFT-based correlation (`Mode::Valid`) for efficient chirp template matching:
- O(N log N) complexity instead of O(N²)
- Pre-computed chirp templates for both up and down directions
- Peak correlation value determines bit value

## Performance Characteristics

### Throughput
- **Raw bit rate**: ~20 bits/second (1 bit per 50ms symbol)
- **After FEC overhead (255→223)**: ~17.5 bits/second of payload
- **Typical message**: 100 bytes takes ~45 seconds to transmit

### Latency
- **Preamble detection**: ~250ms (preamble length)
- **Symbol detection**: ~50ms per symbol
- **Postamble detection**: ~250ms (postamble length)
- **Total minimum**: ~550ms + transmission time

### Robustness
- Handles noise well due to continuous modulation
- Good performance in multipath environments
- Resistant to frequency offset (chirp is self-correcting)
- Degrades gracefully with increasing noise

## Implementation Notes

### CssModulator
- Generates chirp signals for each bit
- Takes boolean array of bits as input
- Returns concatenated float32 samples

### CssDemodulator
- Correlates input with chirp templates
- Uses FFT-based correlation for efficiency
- Decides bits based on correlation strength
- Handles symbol boundary detection

### EncoderCss / DecoderCss
- Wrapper classes that add framing, FEC, and sync detection
- Compatible with existing infrastructure
- Produces standard WAV files at 16kHz, 16-bit mono

## Comparison with Quiet Library

The Quiet library uses similar principles:
- Chirp-based modulation for smooth sound
- Reed-Solomon FEC for error correction
- Frame-based structure with sync markers
- CSS is similar to Quiet's "ultrasonic" mode

## Limitations

- Lower throughput than OFDM (but more robust)
- Longer transmission times for same data
- Symbol rate is fixed at 20 bits/second
- Requires full preamble/postamble detection (no adaptive sync)

## Future Improvements

Potential enhancements to CSS implementation:
- Variable symbol duration for throughput/robustness tradeoff
- Multiple chirp rates for faster transmission
- Adaptive demodulation based on SNR
- Channel equalization for frequency-selective fading
- Interleaving for burst error correction
