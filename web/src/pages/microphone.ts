/**
 * Microphone preamble detection page component
 */

import { initWasm, PreambleDetector } from '../utils/wasm';

export async function MicrophonePage(): Promise<string> {
    const html = `
        <div class="text-center mb-5">
            <h1>🎤 Preamble Detection</h1>
            <p>Real-time detection of ascending chirp preamble (200Hz → 4000Hz)</p>
        </div>

        <div class="card">
            <h2>Microphone Settings</h2>

            <div class="mt-4">
                <label><strong>Microphone Volume</strong></label>
                <div class="flex items-center gap-3 mt-2">
                    <input type="range" id="volumeSlider" min="0.5" max="3" step="0.1" value="1" />
                    <span id="volumeValue">1.0x</span>
                </div>
                <small>Amplify microphone input (0.5x to 3x). Recommended: 1.0x</small>
            </div>

            <div class="mt-4">
                <label><strong>Input Level</strong></label>
                <div style="background: #f7fafc; padding: 1rem; border-radius: 0.5rem; margin-top: 0.5rem;">
                    <div style="display: flex; gap: 0.5rem; height: 20px; background: #e2e8f0; border-radius: 4px; overflow: hidden;">
                        <div id="volumeBar" style="background: linear-gradient(90deg, #4ade80, #facc15, #ef4444); height: 100%; width: 0%; transition: width 0.05s linear;"></div>
                    </div>
                    <div style="margin-top: 0.5rem; font-size: 0.85rem; color: #666;">
                        Peak: <span id="peakLevel">0.0</span> dB
                    </div>
                </div>
            </div>

            <div class="mt-4">
                <label><strong>Detection Threshold</strong></label>
                <div class="flex items-center gap-3 mt-2">
                    <input type="range" id="thresholdSlider" min="0.1" max="0.9" step="0.1" value="0.4" />
                    <span id="thresholdValue">0.4</span>
                </div>
                <small>Higher values require stronger preamble detection. Recommended: 0.4</small>
            </div>

            <div class="mt-4">
                <button id="startBtn" class="btn-primary w-full">Start Listening</button>
                <button id="stopBtn" class="btn-secondary w-full mt-3" style="display: none;">Stop Listening</button>
            </div>

            <div id="status" class="mt-4" style="display: none;"></div>

            <div id="bufferInfo" class="mt-4" style="display: none;">
                <p><strong>Buffer Status:</strong></p>
                <div style="background: #f7fafc; padding: 1rem; border-radius: 0.5rem;">
                    <div>Buffer: <span id="bufferSize">0</span> / <span id="requiredSize">4000</span> samples</div>
                    <div style="background: var(--border-color); height: 8px; border-radius: 4px; margin-top: 0.5rem;">
                        <div id="bufferBar" style="background: var(--primary-color); height: 100%; border-radius: 4px; width: 0%; transition: width 0.2s;"></div>
                    </div>
                </div>
            </div>

            <div id="detectionHistory" class="mt-4" style="display: none;">
                <p><strong>Detection History:</strong></p>
                <div id="historyList" style="
                    background: #f7fafc;
                    padding: 1rem;
                    border-radius: 0.5rem;
                    max-height: 200px;
                    overflow-y: auto;
                    font-family: monospace;
                    font-size: 0.9rem;
                "></div>
            </div>
        </div>

        <a href="#/" style="color: var(--primary-color); text-decoration: none; font-weight: 500; margin-top: 2rem; display: inline-block;">
            ← Back to Home
        </a>
    `;

    const root = document.getElementById('app')!;
    root.innerHTML = html;

    setupMicrophoneListeners();

    return html;
}

function setupMicrophoneListeners(): void {
    const startBtn = document.getElementById('startBtn') as HTMLButtonElement;
    const stopBtn = document.getElementById('stopBtn') as HTMLButtonElement;
    const volumeSlider = document.getElementById('volumeSlider') as HTMLInputElement;
    const volumeValue = document.getElementById('volumeValue') as HTMLElement;
    const thresholdSlider = document.getElementById('thresholdSlider') as HTMLInputElement;
    const thresholdValue = document.getElementById('thresholdValue') as HTMLElement;
    const status = document.getElementById('status')!;
    const bufferInfo = document.getElementById('bufferInfo')!;
    const detectionHistory = document.getElementById('detectionHistory')!;

    let detector: PreambleDetector | null = null;
    let isListening = false;
    let gainNode: GainNode | null = null;
    let analyser: AnalyserNode | null = null;
    let peakLevel = 0;

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
            detector = new PreambleDetector(threshold);

            // Request microphone access
            const stream = await navigator.mediaDevices.getUserMedia({ audio: true });

            const audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
            const source = audioContext.createMediaStreamSource(stream);
            gainNode = audioContext.createGain();
            analyser = audioContext.createAnalyser();
            const processor = audioContext.createScriptProcessor(4096, 1, 1);

            // Set initial gain
            gainNode.gain.value = parseFloat(volumeSlider.value);
            analyser.fftSize = 2048;

            // Connect: microphone -> gain -> analyser -> processor -> output
            source.connect(gainNode);
            gainNode.connect(analyser);
            analyser.connect(processor);
            processor.connect(audioContext.destination);

            isListening = true;
            startBtn.style.display = 'none';
            stopBtn.style.display = 'block';
            bufferInfo.style.display = 'block';
            detectionHistory.style.display = 'block';

            showStatus(status, 'Listening for preamble...', 'info');

            const detections: string[] = [];
            const requiredSize = PreambleDetector.required_size();

            processor.onaudioprocess = (event: any) => {
                const samples = event.inputData.getChannelData(0);

                // Update volume meter
                if (analyser) {
                    const dataArray = new Uint8Array(analyser.frequencyBinCount);
                    analyser.getByteFrequencyData(dataArray);
                    const average = dataArray.reduce((a, b) => a + b) / dataArray.length;
                    const db = 20 * Math.log10(average / 128 + 0.0001);
                    peakLevel = Math.max(peakLevel * 0.95, db); // Decay peak

                    const volumeBar = document.getElementById('volumeBar')!;
                    const peakDisplay = document.getElementById('peakLevel')!;
                    const normalizedDb = Math.max(0, Math.min(100, (db + 60) / 0.6)); // Map -60dB to 0dB range
                    volumeBar.style.width = normalizedDb + '%';
                    peakDisplay.textContent = db.toFixed(1);
                }

                const position = detector!.add_samples(samples);

                // Update buffer info
                const bufferSize = detector!.buffer_size();
                const bufferBar = document.getElementById('bufferBar')!;
                const bufferSizeEl = document.getElementById('bufferSize')!;
                const requiredSizeEl = document.getElementById('requiredSize')!;

                bufferBar.style.width = ((bufferSize / requiredSize) * 100) + '%';
                bufferSizeEl.textContent = bufferSize.toString();
                requiredSizeEl.textContent = requiredSize.toString();

                // Handle detection
                if (position >= 0) {
                    const timestamp = new Date().toLocaleTimeString();
                    detections.unshift(`${timestamp}: Detected at position ${position}`);
                    detections.splice(10); // Keep only last 10

                    const historyList = document.getElementById('historyList')!;
                    historyList.innerHTML = detections.map(d => `<div>${d}</div>`).join('');

                    showStatus(status, 'Preamble detected!', 'success');
                    setTimeout(() => {
                        showStatus(status, 'Listening for preamble...', 'info');
                    }, 2000);
                }
            };

            // Stop button
            stopBtn.addEventListener('click', () => {
                processor.disconnect();
                source.disconnect();
                stream.getTracks().forEach(track => track.stop());

                isListening = false;
                startBtn.style.display = 'block';
                stopBtn.style.display = 'none';
                showStatus(status, 'Stopped listening', 'info');
            }, { once: true });
        } catch (error) {
            showStatus(
                status,
                `Error: ${error instanceof Error ? error.message : 'Failed to access microphone'}`,
                'error'
            );
        }
    });
}

function showStatus(element: HTMLElement, message: string, type: 'success' | 'error' | 'info'): void {
    element.className = `status status-${type}`;
    element.textContent = message;
    element.style.display = 'block';
}
