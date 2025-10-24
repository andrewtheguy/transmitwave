/**
 * Microphone preamble detection page component
 */

import { initWasm, MicrophoneListener } from '../utils/wasm';

export async function MicrophonePage(): Promise<string> {
    const html = `
        <div class="text-center mb-5">
            <h1>🎤 Preamble Detection</h1>
            <p>Real-time detection of ascending chirp preamble (200Hz → 4000Hz)</p>
        </div>

        <div class="card">
            <h2>Microphone Settings</h2>

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
    const thresholdSlider = document.getElementById('thresholdSlider') as HTMLInputElement;
    const thresholdValue = document.getElementById('thresholdValue') as HTMLElement;
    const status = document.getElementById('status')!;
    const bufferInfo = document.getElementById('bufferInfo')!;
    const detectionHistory = document.getElementById('detectionHistory')!;

    let listener: MicrophoneListener | null = null;
    let isListening = false;

    // Update threshold display
    thresholdSlider.addEventListener('input', () => {
        thresholdValue.textContent = thresholdSlider.value;
    });

    // Start button
    startBtn.addEventListener('click', async () => {
        try {
            await initWasm();

            const threshold = parseFloat(thresholdSlider.value);
            listener = new MicrophoneListener(threshold);

            // Request microphone access
            const stream = await navigator.mediaDevices.getUserMedia({ audio: true });

            const audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
            const source = audioContext.createMediaStreamSource(stream);
            const processor = audioContext.createScriptProcessor(4096, 1, 1);

            source.connect(processor);
            processor.connect(audioContext.destination);

            isListening = true;
            startBtn.style.display = 'none';
            stopBtn.style.display = 'block';
            bufferInfo.style.display = 'block';
            detectionHistory.style.display = 'block';

            showStatus(status, 'Listening for preamble...', 'info');

            const detections: string[] = [];
            const requiredSize = listener.required_size();

            processor.onaudioprocess = (event: AudioProcessingEvent) => {
                const samples = event.inputData.getChannelData(0);
                const position = listener!.add_samples(samples);

                // Update buffer info
                const bufferSize = listener!.buffer_size();
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
