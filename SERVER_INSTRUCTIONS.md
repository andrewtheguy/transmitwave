# Audio Modem Web Demo - Server Instructions

## Quick Start

### Option 1: Using Python (Recommended)

```bash
cd /Users/it3/codes/andrew/testaudio
python3 -m http.server 8000
```

Then open your browser and navigate to: **http://localhost:8000/demo.html**

### Option 2: Using Node.js http-server

If you have `http-server` installed globally:

```bash
cd /Users/it3/codes/andrew/testaudio
http-server
```

### Option 3: Using macOS SimpleHTTPServer

```bash
cd /Users/it3/codes/andrew/testaudio
python3 -m http.server 8000 --bind 127.0.0.1
```

## Demo Features

### Encoding (Text to Audio)
1. Enter text (up to 200 characters) in the input field
2. Click "Encode to Audio"
3. Listen to the preview or download as WAV file
4. The audio contains:
   - 250ms ascending chirp preamble (200Hz → 4000Hz)
   - OFDM-encoded data with Reed-Solomon error correction
   - 250ms descending chirp postamble (4000Hz → 200Hz)

### Decoding (Audio to Text)
1. Upload a WAV file (preferably encoded with this demo)
2. Click "Decode Audio"
3. The decoded text appears in the results
4. Works with files that have:
   - Leading/trailing silence
   - Background noise (up to ~20% amplitude)
   - Mono or stereo audio

## Technical Details

### Audio Encoding
- **Sample Rate**: 16,000 Hz
- **Symbol Duration**: 100ms
- **OFDM Subcarriers**: 48 (spaced 79Hz apart)
- **Frequency Range**: 200Hz - 4000Hz
- **Preamble**: 250ms chirp (ascending)
- **Postamble**: 250ms chirp (descending)
- **FEC**: Reed-Solomon (255, 223) - corrects up to 16 byte errors
- **Max Payload**: 200 bytes

### Frame Structure
```
[Silence] [Preamble] [Data] [Postamble] [Silence]
  (opt)    (250ms)   (var)   (250ms)    (opt)
```

## Browser Compatibility

- Chrome/Edge: ✅ Full support
- Firefox: ✅ Full support
- Safari: ✅ Full support (iOS 14.5+)
- Opera: ✅ Full support

Requires WebAssembly support (all modern browsers).

## Troubleshooting

### WASM Module Not Found
- Ensure WASM is built: `cd wasm && wasm-pack build --target web`
- Clear browser cache (Ctrl+Shift+Del or Cmd+Shift+Del)
- Check browser console for errors (F12)

### Decoding Fails
- Use audio encoded by this demo for best results
- Ensure audio is mono or will be converted automatically
- Check that the WAV file is not corrupted
- Try normalizing audio levels if decoding fails

### Audio Quality Issues
- Lower amplitude noise (< 10%) should decode perfectly
- Higher noise (10-20%) may require repeated attempts
- Use the encoded WAV files directly for guaranteed success

## File Structure

```
/testaudio
├── demo.html              # Web interface
├── wasm/
│   ├── src/lib.rs        # WASM bindings
│   └── pkg/              # Compiled WASM (auto-generated)
│       ├── testaudio_wasm_bg.wasm
│       ├── testaudio_wasm.js
│       └── package.json
└── core/                 # Rust audio modem library
    ├── src/
    │   ├── lib.rs
    │   ├── encoder.rs
    │   ├── decoder.rs
    │   ├── ofdm.rs
    │   ├── fec.rs
    │   ├── sync.rs
    │   └── framing.rs
    └── tests/
```

## Building WASM

To rebuild the WASM module after changing Rust code:

```bash
cd /Users/it3/codes/andrew/testaudio/wasm
wasm-pack build --target web
```

This generates optimized WASM and JavaScript bindings in the `pkg/` directory.

## Example Use Cases

1. **Offline Data Transfer**: Encode documents as audio and transmit over limited bandwidth channels
2. **Acoustic Covert Channels**: Use audio for stealth data transmission
3. **IoT Communication**: Send commands via audio without expensive RF hardware
4. **Testing**: Validate audio modem implementation with various audio files
5. **Education**: Learn OFDM, FEC, and synchronization techniques

## Performance Notes

- Encoding: ~100-200ms for typical text
- Decoding: ~500-1000ms for typical messages
- Audio duration: ~2-5 seconds per message
- All processing happens in the browser (no server required)

## Security Considerations

- No data is sent to any server (100% client-side processing)
- Audio files can be inspected and validated before decoding
- Recommended for authorized uses only (see CTF/security context)
