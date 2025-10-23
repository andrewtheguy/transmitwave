# Acoustic Recording Gap Loss Analysis

## Your Observation

When recording or transmitting audio over acoustic channels, the small-amplitude gaps between OFDM symbols disappear because:
- Audio recording devices have a **noise floor** (minimum detectable amplitude)
- Microphones can't record signals below ~0.01-0.1 amplitude (typical)
- The OFDM guard interval tails decay below this threshold
- Result: **Data loss during recording/transmission**

This is a real problem for over-the-air acoustic communication.

## Root Cause Analysis

### Current Frame Structure
```
[Chirp Preamble] [OFDM Data Symbols] [Chirp Postamble]
[4000 samples]   [variable]          [4000 samples]

Each OFDM symbol: 1600 samples (100ms)
Each symbol has:
  - Active region: ~1200 samples (strong signal)
  - Tail region: ~400 samples (exponential decay from FFT)

The tail decays to: amplitude ≈ SAMPLES_PER_SYMBOL^(-1/2) × signal
                 ≈ (1600)^(-0.5) × 1.0 ≈ 0.025

At typical audio levels (scaled to [-1, 1]), this is barely above noise floor.
```

### Recording Loss Mechanism
```
Original signal:          Recording threshold:   After recording:
|‾‾‾|‾‾‾|              |-----------|           |‾‾‾|   |‾‾‾|
|   |   |    ←Decays→   |....|....|           |   | X |   |
|___|___|              |           |           |___|___|

The gaps disappear during recording, breaking symbol timing.
```

## Solutions Ranked by Feasibility

### Solution 1: Increase OFDM Signal Amplitude (SIMPLEST)
**Recommendation: Do this first**

Currently OFDM uses scaling `1/sqrt(SAMPLES_PER_SYMBOL)` ≈ 0.025 for normalization.

**Fix:**
```rust
// In ofdm.rs, modulate function:
let scale = 1.0 / (SAMPLES_PER_SYMBOL as f32).sqrt();
// Change to:
let scale = 4.0 / (SAMPLES_PER_SYMBOL as f32).sqrt(); // 4x amplification
```

**Trade-offs:**
- ✅ Simple one-line fix
- ✅ No architectural changes needed
- ✅ Tails rise above noise floor
- ✅ Maintains all error correction
- ❌ Slight increase in clipping risk (but recoverable with windowing)
- ❌ May saturate poor audio equipment

**Implementation effort:** 5 minutes
**Test:** Verify round-trip still works, measure tail amplitude

### Solution 2: Cyclic Prefix (PROFESSIONAL)
**Recommended for production**

Replace the decaying tail with an explicit guard interval.

**How it works:**
```
Standard OFDM symbol (1600 samples):
[Active Data] [Natural FFT Tail - Decays] [Gap] [Next Symbol]

With Cyclic Prefix (CP):
[CP: Last 160 samples copied] [Active Data] [Next Symbol]
```

**Advantages:**
- ✅ Eliminates ISI (Inter-Symbol Interference)
- ✅ Converts linear convolution → circular convolution
- ✅ Professional standard (WiFi, LTE use this)
- ✅ No amplitude loss to recording devices

**Disadvantages:**
- ❌ Reduces throughput ~10% (160 samples overhead)
- ❌ Moderate implementation effort (2-3 hours)
- ❌ Requires both encoder/decoder changes

**Implementation approach:**
1. Save last 160 samples of OFDM symbol
2. Prepend to next symbol's output
3. Decoder processes symbols with guard prefix
4. Update FFT window handling

### Solution 3: Overlap-Add OFDM (COMPLEX)
**Not recommended for initial implementation**

Use 50% overlap to keep signal continuous.

**Trade-offs:**
- ✅ Eliminates gaps completely
- ✅ Used in some streaming protocols
- ❌ Very complex demodulation (requires perfect reconstruction)
- ❌ Sensitive to timing jitter
- ❌ High implementation complexity (5+ hours debugging)
- ❌ More susceptible to ISI than cyclic prefix

### Solution 4: Increase Sampling Rate (INEFFICIENT)
Use higher sample rate (32 kHz instead of 16 kHz) to reduce tail decay.

**Trade-offs:**
- ✅ Mathematically reduces tail decay percentage
- ❌ Doubles bandwidth requirements
- ❌ Requires hardware changes
- ❌ No fundamental benefit

### Solution 5: Variable Gain Control
Boost low-amplitude regions dynamically.

**Not recommended** - introduces distortion and makes decoder more complex.

---

## Recommended Implementation Path

### Phase 1: Short Term (5 minutes)
**Increase OFDM amplitude 4x** to rise above recording noise floor.

```rust
// core/src/ofdm.rs, line 44
let scale = 4.0 / (SAMPLES_PER_SYMBOL as f32).sqrt();
```

Then test:
- ✅ Round-trip encoding/decoding still works
- ✅ Audio levels reasonable (not clipping)
- ✅ Tail regions now visible in recordings
- ✅ No decoder changes needed

**Validation test:**
```rust
#[test]
fn test_tail_above_noise_floor() {
    // Verify amplitude tail is > 0.05
    // Simulate 10 dB noise floor
    assert!(tail_amplitude > 0.05);
}
```

### Phase 2: Medium Term (2-3 hours if needed)
**Add Cyclic Prefix for professional robustness**

Only if Phase 1 still shows issues or you need:
- Guaranteed ISI immunity
- Multi-path acoustic channels
- Professional grade reliability

### Phase 3: Long Term
**Consider full Overlap-Add** only if:
- You need maximum bandwidth efficiency
- You have time for extensive testing
- Multi-path environment is severe

---

## Technical Details: OFDM Amplitude

### Why Is OFDM So Quiet?

The normalization factor `1/sqrt(SAMPLES_PER_SYMBOL)` comes from energy conservation:

```
Frequency domain: 48 subcarriers, each amplitude 1.0
Total frequency energy: 48

Time domain (after IFFT): 1600 samples
Energy is distributed across all samples
Per-sample amplitude: sqrt(48/1600) ≈ 0.173
Normalized to [-1,1]: 0.173 / SAMPLES_PER_SYMBOL ≈ 0.025
```

This is **correct for energy conservation** but makes the signal fragile in recording.

### Safe Amplification Levels

```
Scale factor 1.0x: Amplitude ≈ 0.025    (gets lost in recording)
Scale factor 2.0x: Amplitude ≈ 0.05     (borderline)
Scale factor 4.0x: Amplitude ≈ 0.1      (solid, above noise floor)
Scale factor 8.0x: Amplitude ≈ 0.2      (strong, safe margin)
Scale factor 10.0x: Amplitude ≈ 0.25    (excellent margin)
```

Recommended: **4-10x amplification**

Trade-off: Slightly higher clipping risk with cheap audio equipment, but data still recovers.

### Clipping Behavior with Higher Amplitude

If amplitude exceeds 1.0:
- Good audio equipment: Soft clipping (recoverable)
- Cheap equipment: Hard clipping (data loss)

Test on your target hardware first.

---

## Implementation Check List

### For Phase 1 (Amplitude Fix)
- [ ] Modify OFDM scale factor in `core/src/ofdm.rs`
- [ ] Test round-trip encoding/decoding
- [ ] Verify amplitude in waveform viewer
- [ ] Check that tails are now visible
- [ ] Verify no clipping on your audio equipment
- [ ] Commit changes

### For Phase 2 (Cyclic Prefix)
- [ ] Analyze acoustic channel characteristics
- [ ] Design cyclic prefix length
- [ ] Modify encoder to prepend CP
- [ ] Modify decoder to handle CP
- [ ] Test with simulated multipath
- [ ] Measure throughput reduction
- [ ] Verify ISI elimination

---

## Conclusion

Your observation about the recording gap loss is **correct and important for acoustic communication**.

**Recommended immediate action:** Increase OFDM amplitude by 4-10x to lift the signal above recording noise floors. This is a simple one-line change that solves the problem without architectural changes.

If you need maximum robustness for noisy acoustic channels, **implement Cyclic Prefix** afterward.

---

## References

- OFDM amplitude normalization: Digital Signal Processing textbooks
- Cyclic prefix: IEEE 802.11 WiFi standard
- Acoustic noise floors: Typical microphone specs (0.01-0.1 amplitude in normalized range)
- Recording artifacts: Audio engineering references on signal-to-noise ratio
