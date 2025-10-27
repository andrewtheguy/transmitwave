import { PreambleDetector, initWasm } from '../utils/wasm'

interface InitMessage {
  type: 'init'
  threshold: number
}

interface AddSamplesMessage {
  type: 'add_samples'
  samples: Float32Array
}

interface ClearMessage {
  type: 'clear'
}

type WorkerMessage = InitMessage | AddSamplesMessage | ClearMessage

let detector: PreambleDetector | null = null
let isInitialized = false
let wasmInitialized = false
const sampleBuffer: Float32Array[] = []

// No eager initialization - wait for explicit init message to avoid race conditions

self.onmessage = async (event: MessageEvent<WorkerMessage>) => {
  try {
    const { type } = event.data

    switch (type) {
      case 'init': {
        const { threshold } = event.data as InitMessage

        // Initialize WASM first (only if not already initialized)
        if (!wasmInitialized) {
          try {
            await initWasm()
            wasmInitialized = true
            console.log('WASM initialized in preamble worker')
          } catch (error) {
            console.error('Failed to initialize WASM in preamble worker:', error)
            self.postMessage({ type: 'error', error: `WASM initialization failed: ${error}` })
            return
          }
        }

        // Clean up old detector if it exists
        if (detector) {
          try {
            detector.free()
          } catch (e) {
            console.warn('Failed to free old detector:', e)
          }
          detector = null
        }

        detector = new PreambleDetector(threshold)
        isInitialized = true
        console.log(`Preamble detector initialized with threshold ${threshold}`)
        self.postMessage({ type: 'init_done' })

        // Process any buffered samples
        if (sampleBuffer.length > 0) {
          console.log(`Processing ${sampleBuffer.length} buffered sample chunks`)
          for (const bufferedSamples of sampleBuffer) {
            if (detector) {
              const position = detector.add_samples(bufferedSamples)
              if (position >= 0) {
                console.log(`Preamble detected in buffered samples at position ${position}!`)
                self.postMessage({ type: 'preamble_detected', position })
                sampleBuffer.length = 0 // Clear buffer after detection
                return
              }
            }
          }
          console.log(`No preamble found in buffered samples`)
        }
        sampleBuffer.length = 0
        break
      }

      case 'add_samples': {
        const { samples } = event.data as AddSamplesMessage

        // If not initialized yet, buffer the samples
        if (!isInitialized || !detector) {
          sampleBuffer.push(samples)
          console.log(`Buffering samples (${sampleBuffer.length} chunks), total: ${sampleBuffer.reduce((sum, s) => sum + s.length, 0)} samples`)
          return
        }

        try {
          const position = detector.add_samples(samples)

          // position >= 0 means preamble was detected
          if (position >= 0) {
            self.postMessage({ type: 'preamble_detected', position })
          }
        } catch (error) {
          // Catch any WASM errors (including FFT planner issues)
          console.error('Error during preamble detection add_samples:', error)
          self.postMessage({ type: 'error', error: `Preamble detection error: ${error}` })
        }
        break
      }

      case 'clear': {
        if (detector) {
          try {
            detector.clear()
          } catch (e) {
            console.error('Error clearing detector:', e)
          }
        }
        sampleBuffer.length = 0
        self.postMessage({ type: 'clear_done' })
        break
      }

      default:
        self.postMessage({ type: 'error', error: `Unknown message type: ${type}` })
    }
  } catch (error) {
    self.postMessage({ type: 'error', error: String(error) })
  }
}
