/**
 * Audio utilities for WAV file handling, resampling, and format conversion
 */

/**
 * Convert stereo audio to mono by averaging left and right channels
 */
export function stereoToMono(stereo: number[]): number[] {
    const mono: number[] = [];
    for (let i = 0; i < stereo.length; i += 2) {
        if (i + 1 < stereo.length) {
            mono.push((stereo[i] + stereo[i + 1]) / 2);
        }
    }
    return mono;
}

/**
 * Resample audio to a target sample rate using linear interpolation
 */
export function resampleAudio(
    samples: number[],
    fromRate: number,
    toRate: number
): number[] {
    if (fromRate === toRate) {
        return samples;
    }

    const ratio = toRate / fromRate;
    const newLength = Math.ceil(samples.length * ratio);
    const resampled: number[] = [];

    for (let i = 0; i < newLength; i++) {
        const srcIdx = i / ratio;
        const srcIdxFloor = Math.floor(srcIdx);
        const srcIdxCeil = srcIdxFloor + 1;
        const fraction = srcIdx - srcIdxFloor;

        let interpolated: number;
        if (srcIdxCeil < samples.length) {
            // Linear interpolation
            interpolated =
                samples[srcIdxFloor] * (1 - fraction) +
                samples[srcIdxCeil] * fraction;
        } else {
            interpolated = samples[srcIdxFloor];
        }

        resampled.push(interpolated);
    }

    return resampled;
}

/**
 * Create a WAV file blob from audio samples
 */
export function createWavBlob(
    samples: number[],
    sampleRate: number = 16000,
    channels: number = 1
): Blob {
    const bytesPerSample = 2;
    const blockAlign = channels * bytesPerSample;

    // WAV file structure
    const wavData = new Uint8Array(
        44 + samples.length * bytesPerSample
    );
    const view = new DataView(wavData.buffer);

    const writeString = (offset: number, string: string) => {
        for (let i = 0; i < string.length; i++) {
            view.setUint8(offset + i, string.charCodeAt(i));
        }
    };

    const subChunk2Size = samples.length * blockAlign;
    const chunkSize = 36 + subChunk2Size;

    // RIFF chunk
    writeString(0, 'RIFF');
    view.setUint32(4, chunkSize, true);
    writeString(8, 'WAVE');

    // fmt sub-chunk
    writeString(12, 'fmt ');
    view.setUint32(16, 16, true); // subChunk1Size
    view.setUint16(20, 1, true); // audio format (1 = PCM)
    view.setUint16(22, channels, true); // number of channels
    view.setUint32(24, sampleRate, true); // sample rate
    view.setUint32(28, sampleRate * blockAlign, true); // byte rate
    view.setUint16(32, blockAlign, true); // block align
    view.setUint16(34, 16, true); // bits per sample

    // data sub-chunk
    writeString(36, 'data');
    view.setUint32(40, subChunk2Size, true);

    // Write audio samples
    const offset = 44;
    for (let i = 0; i < samples.length; i++) {
        const sample = Math.max(-1, Math.min(1, samples[i])); // Clamp to [-1, 1]
        view.setInt16(
            offset + i * 2,
            sample < 0 ? sample * 0x8000 : sample * 0x7fff,
            true
        );
    }

    return new Blob([wavData], { type: 'audio/wav' });
}

/**
 * Parse a WAV file and extract audio samples
 */
export function parseWavFile(
    buffer: ArrayBuffer
): { samples: number[]; sampleRate: number; channels: number } | null {
    const view = new DataView(buffer);

    // Check RIFF header
    if (
        String.fromCharCode(view.getUint8(0), view.getUint8(1), view.getUint8(2), view.getUint8(3)) !==
        'RIFF'
    ) {
        return null;
    }

    // Find fmt chunk
    let fmtOffset = -1;
    for (let i = 12; i < buffer.byteLength - 8; i++) {
        if (
            String.fromCharCode(
                view.getUint8(i),
                view.getUint8(i + 1),
                view.getUint8(i + 2),
                view.getUint8(i + 3)
            ) === 'fmt '
        ) {
            fmtOffset = i;
            break;
        }
    }

    if (fmtOffset === -1) return null;

    const audioFormat = view.getUint16(fmtOffset + 8, true);
    const channels = view.getUint16(fmtOffset + 10, true);
    const sampleRate = view.getUint32(fmtOffset + 12, true);
    const bytesPerSample = view.getUint16(fmtOffset + 22, true) / 8;

    // Find data chunk
    let dataOffset = -1;
    for (let i = fmtOffset; i < buffer.byteLength - 4; i++) {
        if (
            String.fromCharCode(
                view.getUint8(i),
                view.getUint8(i + 1),
                view.getUint8(i + 2),
                view.getUint8(i + 3)
            ) === 'data'
        ) {
            dataOffset = i + 8;
            break;
        }
    }

    if (dataOffset === -1) return null;

    const samples: number[] = [];
    const sampleCount = (buffer.byteLength - dataOffset) / (channels * bytesPerSample);

    for (let i = 0; i < sampleCount; i++) {
        for (let c = 0; c < channels; c++) {
            const offset = dataOffset + (i * channels + c) * bytesPerSample;
            let sample: number;

            if (bytesPerSample === 1) {
                sample = (view.getUint8(offset) - 128) / 128;
            } else if (bytesPerSample === 2) {
                sample = view.getInt16(offset, true) / 32768;
            } else if (bytesPerSample === 4) {
                sample = view.getFloat32(offset, true);
            } else {
                continue;
            }

            samples.push(sample);
        }
    }

    return { samples, sampleRate, channels };
}
