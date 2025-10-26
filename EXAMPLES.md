# Usage Examples

## Command-Line Tool

### Basic Encoding

```bash
# Create some test data
echo "Hello, World!" > tmp/message.txt

# Encode to WAV audio file
cargo run -p transmitwave-cli --bin transmitwave -- encode tmp/message.txt tmp/message.wav

# Output:
# Read 14 bytes from message.txt
# Encoded to 74400 audio samples
# Wrote message.wav to 1
```

### Decoding Back

```bash
# Decode the WAV file
cargo run -p transmitwave-cli --bin transmitwave -- decode tmp/message.wav tmp/recovered.txt

# Output:
# Read WAV: 16000 Hz, 1 channels, 32 bits
# Extracted 74400 samples
# Decoded 14 bytes
# Wrote 14 to recovered.txt

# Verify it matches
diff message.txt recovered.txt
# (no output = files are identical)
```
