# transmitwave Web Interface

A modern web application for the transmitwave audio modem system, built with Vite and WebAssembly.

## Overview

This web interface provides an interactive browser-based demonstration of the transmitwave audio modem system. It uses the compiled WebAssembly module from the Rust core library to encode and decode audio messages directly in the browser.

## Features

- **Main Demo**: Encode text to audio and decode audio back to text
- **Preamble Detection**: Real-time detection of the ascending chirp preamble
- **Postamble Detection**: Real-time detection of the descending chirp postamble
- **Live Recording & Decode**: Record from microphone, detect frame boundaries, and decode messages
- **Modern UI**: Responsive design with gradient backgrounds and smooth animations
- **TypeScript**: Full type safety with TypeScript support
- **Hot Module Replacement**: Instant feedback during development

## Project Structure

```
web/
├── src/
│   ├── main.ts                 # Application entry point and router
│   ├── styles/
│   │   └── main.css           # Global styles and utilities
│   ├── utils/
│   │   ├── audio.ts           # Audio processing utilities
│   │   └── wasm.ts            # WASM module initialization
│   └── pages/
│       ├── index.ts           # Landing page
│       ├── demo.ts            # Main encode/decode demo
│       ├── microphone.ts       # Preamble detection
│       ├── postamble.ts        # Postamble detection
│       └── recording-decode.ts # Live recording and decode
├── public/                     # Static assets
├── dist/                       # Build output (generated)
├── index.html                  # HTML entry point
├── vite.config.ts             # Vite configuration
├── tsconfig.json              # TypeScript configuration
├── package.json               # Dependencies and scripts
└── README.md                  # This file
```

## Getting Started

### Prerequisites

- Node.js 18+
- npm or yarn
- The compiled WASM module (`../wasm/pkg/`)

### Installation

```bash
cd web
npm install
```

### Development

Start the development server with hot module replacement:

```bash
npm run dev
```

The application will open at `http://localhost:5173`

### Building for Production

Create an optimized production build:

```bash
npm run build
```

The output will be in the `dist/` directory, ready to be deployed.

### Preview Build

Preview the production build locally:

```bash
npm run preview
```

## Technology Stack

- **Vite** - Fast, modern build tool
- **TypeScript** - Type-safe JavaScript
- **WebAssembly** - Rust core library compiled to WASM
- **HTML5** - Modern web standards
- **CSS3** - Responsive styling

## Usage

### Text Encoding Demo

1. Navigate to the **Main Demo** page
2. Enter text in the "Encode Text to Audio" section (up to 200 characters)
3. Click "Encode to Audio"
4. Listen to the generated audio or download it as a WAV file

### Audio Decoding Demo

1. Click the file input to select a WAV file
2. Click "Decode Audio"
3. The decoded text will appear in the "Decoded Message" section

### Microphone Detection

1. Navigate to the **Microphone Detection** page
2. Adjust the detection threshold (0.1-0.9, default 0.4)
3. Click "Start Listening"
4. Play audio with a preamble (ascending chirp 200Hz→4000Hz)
5. The detection will trigger when the preamble is recognized

### Live Recording & Decode

1. Navigate to the **Live Recording & Decode** page
2. Click "Start Recording"
3. Play encoded audio from another device
4. When preamble and postamble are detected, the message will automatically decode
5. The decoded text appears in the "Decoded Message" section

## Configuration

### WASM Initialization

The WASM module is automatically initialized on first use. Manual initialization is available:

```typescript
import { initWasm } from './utils/wasm';

await initWasm();
```

### Audio Processing

Audio utilities are available for custom processing:

```typescript
import {
  createWavBlob,
  parseWavFile,
  stereoToMono,
  resampleAudio
} from './utils/audio';

// Convert stereo to mono
const mono = stereoToMono(stereoSamples);

// Resample audio
const resampled = resampleAudio(samples, 48000, 16000);

// Create WAV file
const blob = createWavBlob(samples, 16000, 1);
```

## Audio Specifications

- **Sample Rate**: 16,000 Hz
- **Symbol Duration**: 100 ms
- **OFDM Subcarriers**: 48
- **Frequency Band**: 200 Hz - 4000 Hz
- **Preamble**: 250 ms ascending chirp
- **Postamble**: 250 ms descending chirp
- **FEC**: Reed-Solomon (223, 255)
- **Max Payload**: 200 bytes

## Browser Compatibility

- Chrome/Edge 90+
- Firefox 88+
- Safari 14.1+
- Opera 76+

Requires WebAssembly support (all modern browsers).

## Performance

- **JavaScript Bundle**: ~37 KB (gzipped: ~7.7 KB)
- **WASM Module**: ~382 KB
- **CSS**: ~4.8 KB (gzipped: ~1.6 KB)
- **Total**: ~424 KB uncompressed, ~170 KB gzipped

## Development Notes

### Hot Module Replacement

Vite provides HMR for instant feedback during development. Changes to TypeScript files, CSS, and components automatically reload in the browser.

### Source Maps

Source maps are generated for both development and production builds, enabling easy debugging in browser dev tools.

### Type Safety

The project uses strict TypeScript configuration to catch errors at compile time.

## Building for Different Environments

### Development Build

```bash
npm run dev
```

Enables HMR, source maps, and skips minification for faster builds.

### Production Build

```bash
npm run build
```

Optimizes bundle size through minification, tree shaking, and code splitting.

### Preview

```bash
npm run preview
```

Serves the production build locally for testing before deployment.

## Deployment

The `dist/` directory can be deployed to any static hosting service:

- **Netlify**: Connect your repository and set build command to `cd web && npm install && npm run build`
- **Vercel**: Same as Netlify
- **GitHub Pages**: Use GitHub Actions to build and deploy
- **Traditional Hosting**: SCP/FTP the `dist/` folder to your server

## Troubleshooting

### WASM Module Not Loading

Ensure the compiled WASM module exists at `../wasm/pkg/`. If not, rebuild it:

```bash
cd ../wasm
wasm-pack build --target web
```

### Microphone Permission Denied

The browser requires explicit user permission to access the microphone. Grant permission when prompted by the browser.

### Audio Not Playing

- Verify your browser supports Web Audio API
- Check browser console for errors
- Ensure audio output is enabled on your system

### Large File Sizes

The WASM module is large (~382 KB) but only loaded once. Consider using service workers for caching in production.

## Contributing

To contribute improvements to the web interface:

1. Make changes to files in `src/`
2. Test with `npm run dev`
3. Build with `npm run build`
4. Ensure no TypeScript errors: `npm run type-check` (if configured)

## License

Same as the main transmitwave project.
