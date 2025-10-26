/**
 * Inline AudioWorklet processor code as a Blob URL
 * This avoids CORS issues and TypeScript transpilation problems in production
 */

const micProcessorCode = `
/**
 * Minimal audio worklet that forwards mono mic samples to the main thread.
 * We clone the buffer each call to keep the AudioWorklet's internal buffers intact.
 */
class MicProcessor extends AudioWorkletProcessor {
  process(inputs) {
    const [input] = inputs
    const channelData = input && input[0]

    if (channelData && channelData.length > 0) {
      // Copy and transfer samples so the audio graph's internal buffer is untouched.
      const samples = new Float32Array(channelData.length)
      samples.set(channelData)
      this.port.postMessage(samples, [samples.buffer])
    }

    return true
  }
}

registerProcessor('mic-processor', MicProcessor)
`

let micProcessorBlobUrl: string | null = null

/**
 * Get the Blob URL for the mic processor worklet
 * Creates the Blob URL once and reuses it
 */
export function getMicProcessorUrl(): string {
  if (!micProcessorBlobUrl) {
    const blob = new Blob([micProcessorCode], { type: 'application/javascript' })
    micProcessorBlobUrl = URL.createObjectURL(blob)
  }
  return micProcessorBlobUrl
}

/**
 * Clean up the Blob URL when no longer needed
 */
export function revokeMicProcessorUrl(): void {
  if (micProcessorBlobUrl) {
    URL.revokeObjectURL(micProcessorBlobUrl)
    micProcessorBlobUrl = null
  }
}
