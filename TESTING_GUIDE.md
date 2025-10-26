# Web Demo Testing Guide

## 🧪 Comprehensive Testing Instructions

This guide provides step-by-step instructions to test all features of the audio modem web demo.

---

## ✅ Pre-Test Checklist

- [ ] Server running: `python3 -m http.server 8000`
- [ ] Browser open to: `http://localhost:8000`
- [ ] JavaScript console available: Press **F12**
- [ ] Audio working: System volume is on, speakers connected
- [ ] Modern browser: Chrome, Firefox, Safari, or Edge

---

## 🎯 Test 1: Basic Encoding

### Steps
1. Navigate to **http://localhost:8000/demo.html**
2. In the **"Text to Audio"** panel, clear the input field
3. Type: `Hello World`
4. Click **"Encode to Audio"**

### Expected Results
- ✅ Status shows: "Successfully encoded 'Hello World' (XXX samples)"
- ✅ Green success badge appears
- ✅ Audio player becomes visible
- ✅ Audio can be played (press play button)
- ✅ Statistics show duration and sample count
- ✅ Download button becomes enabled

### Verification
- Listen to the audio - it should sound like low-frequency tones and preamble chirps
- Duration should be around 2-3 seconds
- Audio should be clear and audible (400-2300 Hz frequency band)

---

## 🎯 Test 2: Character Counter

### Steps
1. Start with a fresh input field
2. Type characters one at a time
3. Watch the character count display

### Expected Results
- ✅ Counter updates in real-time
- ✅ Counter shows current/max (e.g., "5 / 200")
- ✅ Cannot type beyond 200 characters
- ✅ Backspace/delete properly decrements counter

### Test Cases
- Type 10 characters → shows "10 / 200"
- Type to 200 → shows "200 / 200"
- Try typing more → cursor doesn't advance
- Delete some → counter decreases

---

## 🎯 Test 3: Download WAV File

### Steps
1. Encode a message (see Test 1)
2. Click **"Download WAV"**
3. Find the downloaded file in Downloads folder

### Expected Results
- ✅ WAV file downloads to local folder
- ✅ Filename follows pattern: `audio-modem-TIMESTAMP.wav`
- ✅ File size is ~40-50KB per 10 characters
- ✅ File is a valid WAV (can be played in system player)
- ✅ Status shows: "WAV file downloaded"

### Verification
1. Right-click audio player → Save audio as
2. Compare downloaded file size
3. Open WAV in audio editor (Audacity, etc.)
4. Verify file contains audio data

---

## 🎯 Test 4: Basic Decoding

### Steps
1. Encode a message and download the WAV (Test 1 & 3)
2. Go to **"Audio to Text"** panel
3. Click **"Choose File"**
4. Select the downloaded WAV file
5. Click **"Decode Audio"**

### Expected Results
- ✅ File upload shows filename
- ✅ Status shows: "File loaded: filename.wav (XX KB)"
- ✅ Decode button becomes enabled
- ✅ Status shows: "Successfully decoded: 'Your message'"
- ✅ Green success badge appears
- ✅ Decoded text appears in textarea
- ✅ Statistics display

### Verification
- Decoded text matches original message exactly
- Character count in result matches input length
- No garbage characters or corrupted text

---

## 🎯 Test 5: Round-Trip Encoding/Decoding

### Steps
1. Enter a message: `Test Message 123`
2. Click **"Encode to Audio"**
3. Wait for encoding to complete
4. In Decode panel, upload the downloaded WAV
5. Click **"Decode Audio"**
6. Verify result matches original

### Expected Results
- ✅ Original: `Test Message 123`
- ✅ Decoded: `Test Message 123`
- ✅ Exact match (case-sensitive, including spaces)
- ✅ No data loss

### Test Multiple Messages
| Input | Expected Output | Status |
|-------|-----------------|--------|
| `A` | `A` | ✅ |
| `Hello` | `Hello` | ✅ |
| `123!@#` | `123!@#` | ✅ |
| `UPPERCASE` | `UPPERCASE` | ✅ |
| `lowercase` | `lowercase` | ✅ |
| `Mixed Case 123!` | `Mixed Case 123!` | ✅ |

---

## 🎯 Test 6: Edge Cases

### Empty String Test
1. Clear input field (should be empty)
2. Click **"Encode to Audio"**
3. Expected: Status shows error "Please enter some text"

### Maximum Length Test
1. Type exactly 200 characters
2. Click **"Encode to Audio"**
3. Expected: Encodes successfully
4. Try typing more characters → cursor doesn't advance

### Special Characters Test
```
Input: !@#$%^&*()_+-=[]{}|;:',.<>?/~`
Expected: Same output when decoded
```

### Unicode/Emoji Test (UTF-8)
```
Input: Hello 世界 🎵
Expected: Properly encoded/decoded (or error if not supported)
```

### Whitespace Test
```
Input: "  spaces  at  edges  "
Expected: Exact match including spaces
```

---

## 🎯 Test 7: Multiple Encodes

### Steps
1. Encode message 1: `First message`
2. Listen to audio
3. Download WAV 1
4. Encode message 2: `Second message`
5. Listen to audio (should sound different)
6. Download WAV 2
7. Decode WAV 1 → should get "First message"
8. Decode WAV 2 → should get "Second message"

### Expected Results
- ✅ Each encoding produces unique audio
- ✅ Files don't interfere with each other
- ✅ Correct message recovered from each file
- ✅ No cross-talk or data mixing

---

## 🎯 Test 8: File Format Handling

### Valid Files
1. Encode a message and download (valid WAV)
2. Upload and decode
3. Expected: ✅ Successful decoding

### Invalid Files
1. Try uploading a non-audio file (JPG, TXT, etc.)
2. Click **"Decode Audio"**
3. Expected: ❌ Error message: "Invalid WAV file format"

### Corrupted Files
1. Download a valid WAV
2. Open in hex editor and change a few bytes
3. Try to decode
4. Expected: ❌ Decode fails with error

---

## 🎯 Test 9: Error Messages

### Test Each Error Case

| Action | Expected Error |
|--------|-----------------|
| Click Encode with empty text | "Please enter some text" |
| Upload non-WAV file | "Invalid WAV file format" |
| Click Decode without file | "Please select an audio file" |
| WASM not loaded | "WASM module not ready" |

### Verification
- ✅ Error messages are clear
- ✅ Errors are color-coded (red/pink background)
- ✅ Error message suggests solution when possible

---

## 🎯 Test 10: UI Responsiveness

### Button States
1. Encode button
   - ✅ Enabled when text present
   - ✅ Disabled while encoding
   - ✅ Re-enabled after completion

2. Download button
   - ✅ Disabled initially
   - ✅ Enabled after successful encoding
   - ✅ Disabled after clear/new encode

3. Decode button
   - ✅ Disabled initially
   - ✅ Enabled after file upload
   - ✅ Disabled while decoding

### Input Fields
1. Text input
   - ✅ Accepts typing
   - ✅ Enforces 200 char limit
   - ✅ Updates counter in real-time

2. File input
   - ✅ Opens file dialog
   - ✅ Shows selected filename
   - ✅ Enables decode button

---

## 🎯 Test 11: Status Messages

### Encoding Flow
1. Click Encode
   - ✅ Status: "Encoding..." (with spinner)
   - ✅ Button: Disabled
2. Processing completes
   - ✅ Status: "Successfully encoded..." (green)
   - ✅ Button: Enabled
   - ✅ Download: Enabled

### Decoding Flow
1. Select file
   - ✅ Status: "File loaded: XXX" (blue)
2. Click Decode
   - ✅ Status: "Decoding..." (with spinner)
   - ✅ Button: Disabled
3. Complete
   - ✅ Status: "Successfully decoded..." (green)
   - ✅ Text: Displayed
   - ✅ Button: Re-enabled

---

## 🎯 Test 12: Audio Playback

### Audio Player Features
1. Appears after encoding
2. Has play/pause button
3. Has progress bar
4. Has volume control
5. Shows duration
6. Shows current time

### Playback Test
1. Encode: `Test message`
2. Click play button
3. Listen to entire audio
4. Expected:
   - ✅ Sound starts with preamble (chirp signal)
   - ✅ Middle section has multi-tone FSK signals (400-2300 Hz)
   - ✅ Ends with postamble tone
   - ✅ Total duration ~2-3 seconds
   - ✅ Clear and audible on system speakers

---

## 🎯 Test 13: Browser Console

### Check for Errors
1. Press **F12** to open DevTools
2. Click **Console** tab
3. Perform encode/decode
4. Expected: ✅ No red error messages
5. May see blue info messages (normal)

### Verify WASM Load
1. Open DevTools
2. Click **Network** tab
3. Reload page
4. Expected:
   - ✅ WASM file loads (transmitwave_wasm_bg.wasm)
   - ✅ File size ~300KB
   - ✅ Status 200 (successful)

---

## 🎯 Test 14: Responsive Design

### Desktop Testing
1. Open in full browser window
2. Verify:
   - ✅ Two-panel layout (left & right)
   - ✅ All elements visible
   - ✅ Nice spacing and alignment
   - ✅ Gradient background visible

### Tablet Testing (using DevTools)
1. Press **F12** → Toggle device toolbar
2. Select iPad or tablet size
3. Verify:
   - ✅ Layout adapts to width
   - ✅ Buttons are touchable size
   - ✅ Text is readable
   - ✅ Audio player visible

### Mobile Testing
1. Select iPhone size in DevTools
2. Verify:
   - ✅ Single column layout
   - ✅ Full-width panels
   - ✅ Scrollable content
   - ✅ Large touch targets

---

## 🎯 Test 15: Performance

### Encoding Performance
1. Encode: `A`
   - Expected: < 200ms
2. Encode: `Hello World`
   - Expected: < 200ms
3. Encode: 50 characters
   - Expected: < 200ms
4. Encode: 200 characters (max)
   - Expected: < 200ms

### Decoding Performance
1. Decode clean WAV
   - Expected: < 1 second
2. Decode same WAV again
   - Expected: < 1 second (cached WASM)

### Monitor in DevTools
1. Press **F12** → Performance tab
2. Click record
3. Do encode/decode
4. Stop recording
5. Verify:
   - ✅ No long pauses
   - ✅ Smooth animations
   - ✅ No frame drops

---

## 🎯 Test 16: Statistics Display

### Encoding Stats
After encoding, verify displays:
- ✅ Duration (seconds, 1 decimal)
- ✅ Sample count (number format)

### Decoding Stats
After decoding, verify displays:
- ✅ Duration matches input
- ✅ Sample count matches input
- ✅ Format is consistent

### Example
```
Encode "Hello":
Duration: 2.13s
Samples: 34080

Decode "Hello":
Duration: 2.13s (should match)
Samples: 34080 (should match)
```

---

## 📋 Test Summary Sheet

```
Test 1: Basic Encoding           [ ] Pass [ ] Fail
Test 2: Character Counter        [ ] Pass [ ] Fail
Test 3: Download WAV             [ ] Pass [ ] Fail
Test 4: Basic Decoding           [ ] Pass [ ] Fail
Test 5: Round-Trip               [ ] Pass [ ] Fail
Test 6: Edge Cases               [ ] Pass [ ] Fail
Test 7: Multiple Encodes         [ ] Pass [ ] Fail
Test 8: File Format Handling     [ ] Pass [ ] Fail
Test 9: Error Messages           [ ] Pass [ ] Fail
Test 10: UI Responsiveness       [ ] Pass [ ] Fail
Test 11: Status Messages         [ ] Pass [ ] Fail
Test 12: Audio Playback          [ ] Pass [ ] Fail
Test 13: Browser Console         [ ] Pass [ ] Fail
Test 14: Responsive Design       [ ] Pass [ ] Fail
Test 15: Performance             [ ] Pass [ ] Fail
Test 16: Statistics Display      [ ] Pass [ ] Fail

Total: ___/16 tests passing
```

---

## 🐛 Debugging Tips

### If Tests Fail

#### WASM Module Error
1. Check console (F12 → Console)
2. Look for "WASM module not found"
3. Solution: Verify `/wasm/pkg/` directory exists
4. Rebuild: `cd wasm && wasm-pack build --target web`

#### Encoding Error
1. Check console for exception
2. Verify text input is not empty
3. Check character count < 200
4. Try a simpler message

#### Decoding Error
1. Check console for detailed error
2. Verify file is valid WAV format
3. Try with a file encoded by this demo
4. Check file isn't corrupted

#### Audio Not Playing
1. Check system volume is on
2. Verify speakers/headphones working
3. Try different browser
4. Check F12 Console for errors

---

## ✨ Passing Criteria

Your implementation passes if:

- ✅ All 16 tests pass
- ✅ No red errors in console
- ✅ Audio quality is acceptable
- ✅ Decoding is accurate
- ✅ UI is responsive
- ✅ No crashes or hangs

---

## 🎉 Success!

If you've completed all tests with passing results, the audio modem web demo is working perfectly! 🎵

**Next Steps:**
1. Share with friends
2. Encode secret messages and transmit
3. Modify code and experiment
4. Read DEMO_README.md for deeper understanding

Enjoy! 🚀
