# FSK Shortened Reed-Solomon Optimization

## Problem
Small payloads (e.g., "Hello FSK!" = 11 bytes) were padded to 223 bytes before RS encoding, creating 204 bytes of zero-padding. These zeros encoded to a monotonous 1200 Hz tone lasting ~20 seconds, wasting bandwidth.

## Solution
Implemented **shortened Reed-Solomon encoding** where only actual data + parity bytes are transmitted, not the full 223-byte block.

### Encoding Changes (encoder_fsk.rs)
1. Add 2-byte length prefix indicating frame data length
2. For each RS block:
   - Prepend zeros to reach 223 bytes (for RS encoder)
   - Apply RS encoding → 255 bytes (223 data + 32 parity)
   - **Transmit only** (actual_data_len + 32) bytes (skip prepended zeros)

### Decoding Changes (decoder_fsk.rs)
1. Read 2-byte length prefix
2. For each shortened RS block:
   - Calculate padding needed: 223 - actual_data_len
   - Receive (actual_data_len + 32) bytes
   - Restore to 255 bytes by prepending zeros
   - Apply RS decoding
   - Remove prepended zeros from output

## Performance Improvements

### "Hello FSK!" (11 bytes)

**Before Shortened RS:**
- Frame size: 19 bytes (11 data + 8 header)
- Padded to: 223 bytes (204 bytes of zeros)
- RS encoded: 255 bytes
- Transmission: 255 × 8 = 2,040 bits = 1,020 symbols
- Audio samples: ~424,000 samples
- **Duration: ~26.5 seconds**
- Monotonous tone: ~20 seconds of 1200 Hz

**After Shortened RS:**
- Frame size: 19 bytes
- Length prefix: 2 bytes
- RS encoded: 51 bytes (19 data + 32 parity)
- Total transmitted: 53 bytes
- Transmission: 53 × 8 = 424 bits = 212 symbols
- Audio samples: 94,400 samples
- **Duration: ~5.9 seconds**
- No monotonous tone!

### Improvements
- **77% reduction** in transmission time for small messages
- **4.4x speedup** (26.5s → 5.9s)
- **Eliminated 20-second monotonous tone**
- Still maintains full error correction capability (up to 16 byte errors)

## Technical Details

### Shortened RS Code Theory
Shortened RS(n, k) codes work by:
1. Prepending (255-n) zeros to create standard RS(255, 223) input
2. Encoding normally → 255 bytes
3. Removing the prepended zeros before transmission → n bytes
4. Decoder adds zeros back, decodes, removes zeros again

This maintains the same error correction capability but reduces transmission overhead for small payloads.

### Large Payload Behavior
For payloads > 223 bytes:
- Multiple RS blocks are used
- Only the last block benefits from shortened encoding
- Large payloads still transmit nearly all data

## Verification
✅ All 23 unit tests pass
✅ All 12 integration tests pass
✅ CLI roundtrip verified: "Hello FSK!" → 5.9 seconds
✅ Decoding accuracy: Perfect reconstruction
✅ Error correction: Maintained (32-byte parity per block)

## Files Modified
- `core/src/encoder_fsk.rs`: Implement shortened RS encoding
- `core/src/decoder_fsk.rs`: Implement shortened RS decoding
- `core/src/fsk.rs`: Clean up unused imports
