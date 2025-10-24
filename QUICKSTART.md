# Audio Modem Demo - Quick Start Guide

## 🚀 Launch in 30 Seconds

### Step 1: Start the Server
```bash
cd /Users/it3/codes/andrew/transmitwave
python3 -m http.server 8000
```

### Step 2: Open Your Browser
Visit: **http://localhost:8000**

### Step 3: Start Using!

---

## 📝 Encoding Text to Audio

### Quick Steps
1. Type your message in the **"Text to Audio"** panel (left side)
2. Click **"Encode to Audio"**
3. Listen to the audio preview
4. *(Optional)* Click **"Download WAV"** to save the file

### Example Messages
- "Hello, Audio Modem!" ← Pre-filled example
- "Secret message 123" ← Works great
- "🎵 Works with emojis! 🎵" ← UTF-8 compatible

### Tips
- Maximum 200 characters
- Shorter messages encode faster
- You can encode the same message multiple times
- Audio duration increases with message length (~1.5s base + data)

---

## 🎧 Decoding Audio to Text

### Quick Steps
1. Click **"Choose File"** in the **"Audio to Text"** panel (right side)
2. Select a WAV file
3. Click **"Decode Audio"**
4. View the recovered text

### What Files Work?
✅ **Best**: WAV files encoded by this demo
✅ **Good**: Any WAV file (mono or stereo)
✅ **Maybe**: Files with leading/trailing silence
✅ **Maybe**: Files with background noise (< 20%)

### If Decoding Fails
- Try encoding and downloading a fresh WAV file
- Ensure your audio file is valid WAV format
- Check that file isn't heavily compressed
- Look at browser console (F12) for error details

---

## 🎯 Try These Examples

### Example 1: Basic Round-Trip
```
1. Encode: "Hello World"
2. Download: hello_world.wav
3. Upload: hello_world.wav
4. Decode: Should show "Hello World"
```

### Example 2: Test Robustness
```
1. Encode: "Test 123"
2. Listen to the audio
3. Re-upload the SAME file
4. Should always decode correctly
```

### Example 3: Different Messages
```
1. Encode: "First message"
2. Encode: "Second message"
3. Decode: "First message" ← Download and test
4. Decode: "Second message" ← Download and test
```

---

## 🔧 Troubleshooting Quick Reference

| Problem | Solution |
|---------|----------|
| Can't connect to localhost | Verify server is running (check terminal) |
| WASM module error | Refresh page (Ctrl+R or Cmd+R) |
| Decoding produces garbage | Use a fresh WAV from "Encode to Audio" |
| File upload not working | Ensure it's a valid WAV file |
| No audio output | Check browser volume and speaker settings |
| Slow encoding | This is normal for first-time use |

---

## 📊 What You Should See

### Successful Encoding
```
Status: "Successfully encoded 'Your text' (12345 samples)"
Audio Player: Shows playable audio with controls
Stats: Duration (2.5s), Sample count (40000)
Button: "Download WAV" becomes enabled
```

### Successful Decoding
```
Status: "Successfully decoded: 'Your text'"
Text Area: Shows your recovered message
Stats: Duration (2.5s), Sample count (40000)
Container: Decoded text section appears
```

### What's Normal?
- 1-2 second initial load time (WASM initialization)
- 0.5-1 second encoding time
- 0.5-1 second decoding time
- Audio is ~2-5 seconds long depending on message

---

## 💡 Tips & Tricks

### For Best Results
1. **Use short messages**: 5-50 characters is ideal
2. **Test encoding first**: Verify audio plays before decoding
3. **Clean audio**: Avoid heavily compressed files
4. **Keep files**: Save successful WAV files as reference
5. **Try multiple times**: Different noise samples may help

### Advanced
- **Inspect Network**: Check WASM loading in DevTools (F12 → Network)
- **Console Messages**: See detailed logs in DevTools (F12 → Console)
- **Audio Inspection**: Right-click audio player → Save to examine WAV
- **File Details**: Check file size - should be ~48KB per 10 characters

### For Troubleshooting
1. Open Browser DevTools: **F12** (or **Cmd+Option+I** on Mac)
2. Go to **Console** tab
3. Try encoding/decoding
4. Look for error messages in red
5. Note any error details for debugging

---

## 🎓 Learning More

### Want to Understand How It Works?
Read **DEMO_README.md** for:
- Technical specifications
- How OFDM encoding works
- How synchronization works
- Error correction details
- Testing methodology

### Want to Build It Yourself?
```bash
# Build the WASM module
cd wasm && wasm-pack build --target web

# Run all tests
cargo test

# View test results
cargo test -- --nocapture
```

### Want to Modify It?
Edit these files:
- `core/src/encoder.rs` - Encoding logic
- `core/src/decoder.rs` - Decoding logic
- `core/src/sync.rs` - Synchronization
- `core/src/ofdm.rs` - OFDM modulation
- `wasm/src/lib.rs` - JavaScript interface
- `demo.html` - Web UI

Then rebuild:
```bash
cd wasm && wasm-pack build --target web
```

---

## 🎉 Success Checklist

- [ ] Server is running (`python3 -m http.server 8000`)
- [ ] Browser is open to http://localhost:8000
- [ ] Can see the landing page with "Launch Demo" button
- [ ] Clicked "Launch Demo" and see two-panel interface
- [ ] Typed a test message in left panel
- [ ] Clicked "Encode to Audio" and saw status message
- [ ] Heard/saw audio preview
- [ ] Clicked "Download WAV" (optional)
- [ ] Uploaded the audio file in right panel
- [ ] Clicked "Decode Audio"
- [ ] Saw original message recovered

**If you checked all boxes → You're ready to use Audio Modem! 🎵**

---

## 📞 Common Questions

**Q: Can I use audio from other sources?**
A: Technically yes, but it won't work unless they were encoded by this demo. The format is very specific.

**Q: How long can messages be?**
A: Up to 200 characters. That's roughly 10-20 seconds of audio depending on content.

**Q: Does it work offline?**
A: Yes! Once the page loads, everything works offline. No internet required.

**Q: Is my data secure?**
A: Yes! All processing happens in your browser. No data is sent anywhere.

**Q: Can I share encoded audio?**
A: Yes! Download the WAV and send it to someone. They can upload it to this demo to decode it.

**Q: What if I close the browser?**
A: Start the server again and reopen the page. Your previous messages won't be there, but you can re-encode them.

---

## 🚀 You're All Set!

**Now go encode and decode!**

👉 **http://localhost:8000**

Need more help? Check **DEMO_README.md** for detailed documentation.

Questions? Review the browser console (F12 → Console) for detailed error messages.

Enjoy! 🎵
