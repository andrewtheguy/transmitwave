/**
 * Live recording and decoding page component
 */

import { initWasm, PreambleDetector, PostambleDetector, createDecoder } from '../utils/wasm';
import { resampleAudio, stereoToMono } from '../utils/audio';

export async function RecordingDecodePage(): Promise<string> {
    const html = `
        <div class="text-center mb-5">
            <h1>🎙️ Live Recording & Decode</h1>
            <p>Record from microphone, detect preamble/postamble, and decode the message</p>
        </div>

        <div class="card">
            <h2>Recording Settings</h2>

            <div class="mt-4">
                <label><strong>Microphone Volume</strong></label>
                <div class="flex items-center gap-3 mt-2">
                    <input type="range" id="volumeSlider" min="0.5" max="3" step="0.1" value="1" />
                    <span id="volumeValue">1.0x</span>
                </div>
                <small>Amplify microphone input (0.5x to 3x). Recommended: 1.0x</small>
            </div>

            <div class="mt-4">
                <label><strong>Detection Threshold</strong></label>
                <div class="flex items-center gap-3 mt-2">
                    <input type="range" id="thresholdSlider" min="0.1" max="0.9" step="0.1" value="0.4" />
                    <span id="thresholdValue">0.4</span>
                </div>
            </div>

            <div class="mt-4">
                <button id="startBtn" class="btn-primary w-full">Start Recording</button>
                <button id="stopBtn" class="btn-secondary w-full mt-3" style="display: none;">Stop Recording</button>
            </div>

            <div id="status" class="mt-4" style="display: none;"></div>

            <div id="recordingInfo" class="mt-4" style="display: none;">
                <p><strong>Recording Status:</strong></p>
                <div style="background: #f7fafc; padding: 1rem; border-radius: 0.5rem;">
                    <div>Duration: <span id="duration">0</span>s</div>
                    <div>Samples: <span id="samples">0</span></div>
                </div>
            </div>

            <div id="decodedOutput" class="mt-4" style="display: none;">
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

        <a href="#/" style="color: var(--primary-color); text-decoration: none; font-weight: 500; margin-top: 2rem; display: inline-block;">
            ← Back to Home
        </a>
    `;

    const root = document.getElementById('app')!;
    root.innerHTML = html;

    setupRecordingListeners();

    return html;
}

function setupRecordingListeners(): void {
    const startBtn = document.getElementById('startBtn') as HTMLButtonElement;
    const stopBtn = document.getElementById('stopBtn') as HTMLButtonElement;
    const volumeSlider = document.getElementById('volumeSlider') as HTMLInputElement;
    const volumeValue = document.getElementById('volumeValue') as HTMLElement;
    const thresholdSlider = document.getElementById('thresholdSlider') as HTMLInputElement;
    const thresholdValue = document.getElementById('thresholdValue') as HTMLElement;
    const status = document.getElementById('status')!;
    const recordingInfo = document.getElementById('recordingInfo')!;
    const decodedOutput = document.getElementById('decodedOutput')!;

    let isRecording = false;
    let audioContext: AudioContext;
    let recordedSamples: number[] = [];
    let startTime: number;
    let gainNode: GainNode | null = null;

    // Update volume display
    volumeSlider.addEventListener('input', () => {
        volumeValue.textContent = parseFloat(volumeSlider.value).toFixed(1) + 'x';
        if (gainNode) {
            gainNode.gain.value = parseFloat(volumeSlider.value);
        }
    });

    // Update threshold display
    thresholdSlider.addEventListener('input', () => {
        thresholdValue.textContent = thresholdSlider.value;
    });

    // Start button
    startBtn.addEventListener('click', async () => {
        try {
            await initWasm();

            const threshold = parseFloat(thresholdSlider.value);
            const preDetector = new PreambleDetector(threshold);
            const postDetector = new PostambleDetector(threshold);

            // Request microphone access
            const stream = await navigator.mediaDevices.getUserMedia({ audio: true });

            audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
            const source = audioContext.createMediaStreamSource(stream);
            gainNode = audioContext.createGain();
            const processor = audioContext.createScriptProcessor(4096, 1, 1);

            // Set initial gain
            gainNode.gain.value = parseFloat(volumeSlider.value);

            // Connect: microphone -> gain -> processor -> output
            source.connect(gainNode);
            gainNode.connect(processor);
            processor.connect(audioContext.destination);

            isRecording = true;
            recordedSamples = [];
            startTime = Date.now();

            startBtn.style.display = 'none';
            stopBtn.style.display = 'block';
            recordingInfo.style.display = 'block';

            showStatus(status, 'Waiting for preamble...', 'info');

            let state: 'waiting_preamble' | 'waiting_postamble' | 'complete' = 'waiting_preamble';
            let dataStart = 0;
            let dataEnd = 0;

            processor.onaudioprocess = (event: AudioProcessingEvent) => {
                const samples = Array.from(event.inputData.getChannelData(0));
                recordedSamples.push(...samples);

                if (state === 'waiting_preamble') {
                    const pos = preDetector.add_samples(samples);
                    if (pos >= 0) {
                        dataStart = recordedSamples.length;
                        state = 'waiting_postamble';
                        showStatus(status, 'Preamble detected! Waiting for postamble...', 'success');
                    }
                } else if (state === 'waiting_postamble') {
                    const pos = postDetector.add_samples(samples);
                    if (pos >= 0) {
                        dataEnd = recordedSamples.length - postDetector.buffer_size();
                        state = 'complete';
                        showStatus(status, 'Postamble detected! Decoding...', 'success');

                        // Start decoding automatically
                        processDecode(
                            recordedSamples.slice(dataStart, dataEnd),
                            audioContext.sampleRate
                        );
                    }
                }

                // Update UI
                const duration = Math.floor((Date.now() - startTime) / 1000);
                document.getElementById('duration')!.textContent = duration.toString();
                document.getElementById('samples')!.textContent = recordedSamples.length.toString();
            };

            // Stop button
            stopBtn.addEventListener('click', () => {
                processor.disconnect();
                source.disconnect();
                stream.getTracks().forEach(track => track.stop());

                isRecording = false;
                startBtn.style.display = 'block';
                stopBtn.style.display = 'none';

                if (state !== 'complete') {
                    showStatus(status, 'Recording stopped. No complete frame detected.', 'warning');
                }
            }, { once: true });
        } catch (error) {
            showStatus(
                status,
                `Error: ${error instanceof Error ? error.message : 'Failed to start recording'}`,
                'error'
            );
        }
    });

    async function processDecode(samples: number[], sampleRate: number): Promise<void> {
        try {
            // Resample if needed
            let processedSamples = samples;
            if (sampleRate !== 16000) {
                processedSamples = resampleAudio(samples, sampleRate, 16000);
            }

            const decoder = await createDecoder();
            const data = await decoder.decode(processedSamples);
            const text = new TextDecoder().decode(data);

            const decodedText = document.getElementById('decodedText')!;
            decodedText.textContent = text;
            decodedOutput.style.display = 'block';

            showStatus(status, 'Decoded successfully!', 'success');
        } catch (error) {
            showStatus(
                status,
                `Decode failed: ${error instanceof Error ? error.message : 'Unknown error'}`,
                'error'
            );
        }
    }
}

function showStatus(element: HTMLElement, message: string, type: 'success' | 'error' | 'info' | 'warning'): void {
    element.className = `status status-${type}`;
    element.textContent = message;
    element.style.display = 'block';
}
