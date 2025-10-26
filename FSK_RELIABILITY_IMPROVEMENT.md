# FSK Reliability Improvement

## Current FSK Implementation
Multi-tone FSK uses longer symbol durations to improve noise immunity and frequency resolution in over-the-air audio transmission.

### Technical Details
- **Symbol Duration:** 192ms (3072 samples at 16 kHz) - Normal speed (default)
- **Data Rate:** ~15.6 bytes/second (3 bytes per 192ms symbol)
- **Symbol Constant:** `FSK_SYMBOL_SAMPLES = 3072` (core/src/fsk.rs:67)
- **Alternative Speeds:** 96ms (Fast) and 48ms (Fastest) available via FskSpeed enum

## Benefits of Longer Symbols
1. **Improved Noise Immunity:** Longer integration time in Goertzel algorithm reduces susceptibility to short bursts of noise
2. **Better Frequency Resolution:** More samples per symbol improve frequency discrimination
3. **Maintained Error Correction:** Full 32-byte Reed-Solomon parity still available per block
4. **No Test Changes Needed:** All tests use FSK_SYMBOL_SAMPLES constant, so they automatically adapt

## Performance Impact

### "Hello FSK!" Transmission (with Shortened RS Optimization)
- **Frame Size:** 19 bytes (11 data + 8 header)
- **Length Prefix:** 2 bytes
- **RS Encoded:** 51 bytes (19 data + 32 parity)
- **Total Transmitted:** 53 bytes
- **Audio Samples:** ~94,400 samples (53 bytes × 8 bits × 223 symbols/min)
- **Duration:** ~5.9 seconds at 16 kHz sample rate
- **Tone:** No monotonous tone (shortened RS eliminates zero-padding artifacts)

### Speed Mode Comparison
| Speed | Samples/Symbol | Duration | Data Rate |
|-------|---|---|---|
| Normal | 3072 (192ms) | ~5.9s for "Hello FSK!" | ~15.6 bytes/sec |
| Fast | 1536 (96ms) | ~3.0s for "Hello FSK!" | ~31.2 bytes/sec |
| Fastest | 768 (48ms) | ~1.5s for "Hello FSK!" | ~62.5 bytes/sec |

### General Formula
- Transmission time = (frame_size + length_prefix) / data_rate
- Data rate depends on FskSpeed mode selection

## Robustness Design

### Current Implementation (192ms symbols Normal speed, 15.6 bytes/sec)
- **Extended Integration Time:** 192ms symbol duration provides 3072 samples for Goertzel frequency detection
- **Frequency Resolution:** 96 frequency bins with 20 Hz spacing (400-2300 Hz band)
- **Error Correction:** Reed-Solomon (255, 223) with 32-byte parity per block
- **Detection Method:** Non-coherent energy detection of 6 simultaneous frequencies per symbol
- **Optimized For:** Over-the-air audio transmission in real-world acoustic environments

### Noise Resilience
- Multi-tone simultaneous transmission provides redundancy
- Longer symbol duration improves low-frequency (sub-bass) detection
- Extended integration window reduces susceptibility to noise bursts
- 400-2300 Hz band optimized for speaker/microphone acoustic response
- Demodulator preprocessing removes DC offset, applies a raised-cosine analysis window, and normalizes RMS so Goertzel bins stay comparable even when microphone gain swings wildly
- Per-band median noise-floor subtraction (16-bin groups) suppresses broadband energy before tone selection, which boosts recovery odds when signals fade deep into the noise floor

## Verification

| Check | Status |
|-------|--------|
| `cargo test -p transmitwave-core` | ✅ 114 unit tests passing |
| `cargo test -p transmitwave-core --test integration_test` | ✅ 12 integration tests passing (1 ignored: slow max-payload) |
| `cargo test -p transmitwave-core --test sync_detection_tests` | ✅ 12 sync tests passing |
| CLI roundtrip | ✅ Deterministic |
| New robustness coverage | ✅ Gain-invariance & DC-rejection tests in `core/src/fsk.rs` |

## Implementation Details

### Symbol Configuration (core/src/fsk.rs:67)
```rust
pub const FSK_SYMBOL_SAMPLES: usize = 3072;  // 192ms at 16kHz
```

### Speed Mode Selection (core/src/fsk.rs:40-47)
```rust
pub enum FskSpeed {
    Normal,    // 3072 samples = 192ms (default)
    Fast,      // 1536 samples = 96ms
    Fastest,   // 768 samples = 48ms
}
```

### Data Encoding (core/src/fsk.rs:17-21)
- Transmits 3 bytes (6 nibbles) per symbol
- Each nibble (4 bits) selects one of 16 frequencies
- 6 frequencies transmitted simultaneously for redundancy
- Uses Reed-Solomon FEC for error correction

### No Changes Required
- encoder_fsk.rs (uses FSK_SYMBOL_SAMPLES and FskSpeed)
- decoder_fsk.rs (uses FSK_SYMBOL_SAMPLES and FskSpeed)
- All tests (adapt via FskSpeed configuration)

## Summary
Multi-tone FSK with 192ms normal symbol duration and shortened Reed-Solomon encoding provides an optimal balance between:
- **Robustness:** Extended integration time improves noise immunity
- **Speed:** Shortened RS optimization eliminates zero-padding delays
- **Flexibility:** FskSpeed enum allows speed/robustness trade-off
- **Reliability:** Sub-bass 400-2300 Hz band optimized for over-the-air audio transmission
- **Signal Conditioning:** Automatic gain control + analysis windowing keep the Goertzel inputs stable, while per-band noise-floor subtraction emphasizes the desired tones

### Potential Next Steps
1. Track a rolling per-band noise estimate across consecutive symbols for even better rejection of slowly varying interference.
2. Gather OTA captures at multiple playback volumes to retune AGC targets and taper ratios for the Fast/Fastest modes.
3. Add stress tests that inject real-world impulse noises (claps, keyboard hits) to ensure the conditioning prevents false tone detections.