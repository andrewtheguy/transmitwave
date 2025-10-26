/// <reference lib="webworker" />

/**
 * Minimal audio worklet that forwards mono mic samples to the main thread.
 * We clone the buffer each call to keep the AudioWorklet's internal buffers intact.
 */
class MicProcessor extends AudioWorkletProcessor {
  process(inputs: Float32Array[][]): boolean {
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

export {}
