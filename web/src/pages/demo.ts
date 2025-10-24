/**
 * Main demo page component
 */

import { createEncoder, createDecoder } from '../utils/wasm';
import { createWavBlob, parseWavFile, stereoToMono, resampleAudio } from '../utils/audio';

export async function DemoPage(): Promise<string> {
    const html = `
        <div class="text-center mb-5">
            <h1>Audio Modem Demo</h1>
            <p>Encode text to audio and decode audio back to text</p>
        </div>

        <div class="grid" style="display: grid; grid-template-columns: 1fr 1fr; gap: 2rem; margin-bottom: 2rem;">
            <div class="card">
                <h2>📝 Encode Text to Audio</h2>

                <div class="mt-3">
                    <label><strong>Message</strong></label>
                    <textarea id="encodeInput"
                        placeholder="Enter text to encode..."
                        maxlength="200"
                        style="min-height: 120px; resize: vertical;"></textarea>
                    <div style="text-align: right; margin-top: 0.5rem; font-size: 0.9rem; color: #718096;">
                        <span id="charCount">0</span> / 200 characters
                    </div>
                </div>

                <div class="mt-4">
                    <button id="encodeBtn" class="btn-primary w-full">Encode to Audio</button>
                </div>

                <div id="encodeStatus" class="mt-3" style="display: none;"></div>

                <div id="encodeOutput" class="mt-4" style="display: none;">
                    <p><strong>Encoded Audio:</strong></p>
                    <audio id="encodeAudio" controls style="width: 100%;"></audio>
                    <button id="downloadBtn" class="btn-secondary w-full mt-3">Download WAV</button>
                </div>
            </div>

            <div class="card">
                <h2>🔊 Decode Audio to Text</h2>

                <div class="mt-3">
                    <label><strong>Upload WAV File</strong></label>
                    <input type="file" id="decodeInput" accept=".wav,.mp3" />
                </div>

                <div class="mt-4">
                    <button id="decodeBtn" class="btn-primary w-full">Decode Audio</button>
                </div>

                <div id="decodeStatus" class="mt-3" style="display: none;"></div>

                <div id="decodeOutput" class="mt-4" style="display: none;">
                    <p><strong>Decoded Message:</strong></p>
                    <div id="decodedText" style="
                        background: #f7fafc;
                        padding: 1rem;
                        border-radius: 0.5rem;
                        word-break: break-word;
                        font-family: monospace;
                        min-height: 80px;
                    "></div>
                </div>
            </div>
        </div>

        <a href="#/" style="color: var(--primary-color); text-decoration: none; font-weight: 500;">
            ← Back to Home
        </a>
    `;

    // Create container
    const tempDiv = document.createElement('div');
    tempDiv.innerHTML = html;

    // Wait for DOM to be ready
    const root = document.getElementById('app')!;
    root.innerHTML = html;

    // Setup event listeners
    setupDemoListeners();

    return html;
}

function setupDemoListeners(): void {
    const encodeInput = document.getElementById('encodeInput') as HTMLTextAreaElement;
    const encodeBtn = document.getElementById('encodeBtn') as HTMLButtonElement;
    const decodeInput = document.getElementById('decodeInput') as HTMLInputElement;
    const decodeBtn = document.getElementById('decodeBtn') as HTMLButtonElement;
    const charCount = document.getElementById('charCount') as HTMLElement;
    const downloadBtn = document.getElementById('downloadBtn') as HTMLButtonElement;

    // Character counter
    if (encodeInput && charCount) {
        encodeInput.addEventListener('input', () => {
            charCount.textContent = encodeInput.value.length.toString();
        });
    }

    // Encode button
    if (encodeBtn && encodeInput) {
        encodeBtn.addEventListener('click', async () => {
            await handleEncode(encodeInput.value);
        });
    }

    // Decode button
    if (decodeBtn && decodeInput) {
        decodeBtn.addEventListener('click', async () => {
            const file = decodeInput.files?.[0];
            if (file) {
                await handleDecode(file);
            }
        });
    }

    // Download button
    if (downloadBtn) {
        downloadBtn.addEventListener('click', () => {
            const audio = document.getElementById('encodeAudio') as HTMLAudioElement;
            if (audio && audio.src) {
                const a = document.createElement('a');
                a.href = audio.src;
                a.download = 'encoded-audio.wav';
                a.click();
            }
        });
    }
}

async function handleEncode(text: string): Promise<void> {
    if (!text) {
        showStatus('encodeStatus', 'Please enter text to encode', 'error');
        return;
    }

    const btn = document.getElementById('encodeBtn') as HTMLButtonElement;
    const status = document.getElementById('encodeStatus')!;
    const output = document.getElementById('encodeOutput')!;
    const audio = document.getElementById('encodeAudio') as HTMLAudioElement;

    try {
        btn.disabled = true;
        showStatus('encodeStatus', 'Encoding...', 'info');

        const encoder = await createEncoder();
        const data = new TextEncoder().encode(text);
        const samples = await encoder.encode(data);

        const blob = createWavBlob(samples, 16000, 1);
        audio.src = URL.createObjectURL(blob);
        output.style.display = 'block';
        status.style.display = 'none';

        showStatus('encodeStatus', `Encoded successfully: ${samples.length} samples`, 'success');
    } catch (error) {
        showStatus(
            'encodeStatus',
            `Encoding failed: ${error instanceof Error ? error.message : 'Unknown error'}`,
            'error'
        );
    } finally {
        btn.disabled = false;
    }
}

async function handleDecode(file: File): Promise<void> {
    const btn = document.getElementById('decodeBtn') as HTMLButtonElement;
    const status = document.getElementById('decodeStatus')!;
    const output = document.getElementById('decodeOutput')!;
    const decodedText = document.getElementById('decodedText')!;

    try {
        btn.disabled = true;
        showStatus('decodeStatus', 'Reading file...', 'info');

        const buffer = await file.arrayBuffer();
        const wavData = parseWavFile(buffer);

        if (!wavData) {
            throw new Error('Invalid WAV file');
        }

        let samples = wavData.samples;

        // Convert stereo to mono if needed
        if (wavData.channels > 1) {
            samples = stereoToMono(samples);
        }

        // Resample if needed
        if (wavData.sampleRate !== 16000) {
            samples = resampleAudio(samples, wavData.sampleRate, 16000);
        }

        showStatus('decodeStatus', 'Decoding...', 'info');

        const decoder = await createDecoder();
        const data = await decoder.decode(samples);
        const text = new TextDecoder().decode(data);

        decodedText.textContent = text;
        output.style.display = 'block';
        showStatus('decodeStatus', 'Decoded successfully!', 'success');
    } catch (error) {
        showStatus(
            'decodeStatus',
            `Decoding failed: ${error instanceof Error ? error.message : 'Unknown error'}`,
            'error'
        );
    } finally {
        btn.disabled = false;
    }
}

function showStatus(elementId: string, message: string, type: 'success' | 'error' | 'info' | 'warning'): void {
    const element = document.getElementById(elementId)!;
    element.className = `status status-${type}`;
    element.textContent = message;
    element.style.display = 'block';
}
