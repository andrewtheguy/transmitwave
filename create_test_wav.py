#!/usr/bin/env python3
"""
Create a test WAV file by encoding a message using the Rust core library.
This script demonstrates how to create test audio files.
"""

import subprocess
import struct
import sys

def create_test_wav():
    """Create a test WAV file using cargo run"""
    print("Creating test WAV file...")
    
    # This would require a CLI tool, but for now we'll document how to use the demo
    print("""
    To create test WAV files:
    
    1. Open demo.html in your browser at http://localhost:8000/demo.html
    2. Enter test text in the "Text to Audio" panel
    3. Click "Encode to Audio"
    4. Click "Download WAV"
    5. Use the downloaded file to test decoding
    
    Or, to create via CLI (requires CLI tool):
    cargo run --bin testaudio -- encode "Test message" output.wav
    """)

if __name__ == '__main__':
    create_test_wav()
