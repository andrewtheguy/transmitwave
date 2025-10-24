/**
 * Landing page component
 */

export function IndexPage(): string {
    return `
        <div class="text-center">
            <h1>Testaudio</h1>
            <p style="font-size: 1.25rem; opacity: 0.9;">Audio Modem for Reliable Low-Bandwidth Communication</p>
        </div>

        <div class="grid" style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 2rem; margin-top: 3rem;">
            <a href="#/demo" style="text-decoration: none; color: inherit;">
                <div class="card transition-all" style="cursor: pointer; height: 100%;">
                    <h3>📝 Main Demo</h3>
                    <p>Encode text to audio and decode audio back to text with real-time WAV file generation.</p>
                    <button class="btn-primary w-full">Open Demo</button>
                </div>
            </a>

            <a href="#/microphone" style="text-decoration: none; color: inherit;">
                <div class="card transition-all" style="cursor: pointer; height: 100%;">
                    <h3>🎤 Microphone Detection</h3>
                    <p>Real-time detection of the ascending chirp preamble from your microphone input.</p>
                    <button class="btn-primary w-full">Open Demo</button>
                </div>
            </a>

            <a href="#/postamble" style="text-decoration: none; color: inherit;">
                <div class="card transition-all" style="cursor: pointer; height: 100%;">
                    <h3>🎯 Postamble Detection</h3>
                    <p>Real-time detection of the descending chirp postamble from your microphone input.</p>
                    <button class="btn-primary w-full">Open Demo</button>
                </div>
            </a>

            <a href="#/recording-decode" style="text-decoration: none; color: inherit;">
                <div class="card transition-all" style="cursor: pointer; height: 100%;">
                    <h3>🎙️ Live Recording & Decode</h3>
                    <p>Record from microphone, automatically detect boundaries, and decode the message.</p>
                    <button class="btn-primary w-full">Open Demo</button>
                </div>
            </a>
        </div>

        <div class="card mt-5">
            <h2>Features</h2>
            <ul style="list-style: none; padding: 0;">
                <li style="margin: 0.5rem 0;">✅ Spread spectrum encoding for noise immunity</li>
                <li style="margin: 0.5rem 0;">✅ Reed-Solomon error correction (223→255 bytes)</li>
                <li style="margin: 0.5rem 0;">✅ OFDM with 48 subcarriers (200Hz-4000Hz)</li>
                <li style="margin: 0.5rem 0;">✅ 16kHz sample rate</li>
                <li style="margin: 0.5rem 0;">✅ 200 byte max payload</li>
                <li style="margin: 0.5rem 0;">✅ Real-time preamble/postamble detection</li>
            </ul>
        </div>

        <div class="card mt-5">
            <h2>Technical Specifications</h2>
            <table style="width: 100%; border-collapse: collapse;">
                <tr style="border-bottom: 1px solid var(--border-color);">
                    <td style="padding: 0.5rem;"><strong>Sample Rate</strong></td>
                    <td style="padding: 0.5rem;">16,000 Hz</td>
                </tr>
                <tr style="border-bottom: 1px solid var(--border-color);">
                    <td style="padding: 0.5rem;"><strong>Symbol Duration</strong></td>
                    <td style="padding: 0.5rem;">100 ms</td>
                </tr>
                <tr style="border-bottom: 1px solid var(--border-color);">
                    <td style="padding: 0.5rem;"><strong>OFDM Subcarriers</strong></td>
                    <td style="padding: 0.5rem;">48</td>
                </tr>
                <tr style="border-bottom: 1px solid var(--border-color);">
                    <td style="padding: 0.5rem;"><strong>Frequency Band</strong></td>
                    <td style="padding: 0.5rem;">200 Hz - 4000 Hz</td>
                </tr>
                <tr style="border-bottom: 1px solid var(--border-color);">
                    <td style="padding: 0.5rem;"><strong>Preamble Duration</strong></td>
                    <td style="padding: 0.5rem;">250 ms</td>
                </tr>
                <tr style="border-bottom: 1px solid var(--border-color);">
                    <td style="padding: 0.5rem;"><strong>Postamble Duration</strong></td>
                    <td style="padding: 0.5rem;">250 ms</td>
                </tr>
                <tr style="border-bottom: 1px solid var(--border-color);">
                    <td style="padding: 0.5rem;"><strong>FEC</strong></td>
                    <td style="padding: 0.5rem;">Reed-Solomon (223, 255)</td>
                </tr>
                <tr>
                    <td style="padding: 0.5rem;"><strong>Max Payload</strong></td>
                    <td style="padding: 0.5rem;">200 bytes</td>
                </tr>
            </table>
        </div>
    `;
}
