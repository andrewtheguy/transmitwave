import { PostambleDetector, initWasm } from '../utils/wasm'

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

interface SetThresholdMessage {
  type: 'set_threshold'
  threshold: number
}

type WorkerMessage = InitMessage | AddSamplesMessage | ClearMessage | SetThresholdMessage

let detector: PostambleDetector | null = null
let isInitialized = false
let wasmInitialized = false
let wasmInitPromise: Promise<void> | null = null
let currentThreshold = 0.4
const sampleBuffer: Float32Array[] = []

self.onmessage = async (event: MessageEvent<WorkerMessage>) => {
  try {
    const { type } = event.data

    switch (type) {
      case 'init': {
        const { threshold } = event.data as InitMessage
        currentThreshold = threshold

        // Initialize WASM first (only if not already initialized)
        if (!wasmInitialized) {
          if (wasmInitPromise) {
            try {
              await wasmInitPromise
            } catch (error) {
              console.error('WASM initialization failed in postamble worker:', error)
              self.postMessage({ type: 'error', error: `WASM initialization failed: ${error}` })
              return
            }
          } else {
            wasmInitPromise = initWasm()
            try {
              await wasmInitPromise
              wasmInitialized = true
              console.log('WASM initialized in postamble worker')
            } catch (error) {
              console.error('Failed to initialize WASM in postamble worker:', error)
              self.postMessage({ type: 'error', error: `WASM initialization failed: ${error}` })
              wasmInitPromise = null
              return
            } finally {
              wasmInitPromise = null
            }
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

        detector = new PostambleDetector(currentThreshold)
        isInitialized = true
        console.log(`Postamble detector initialized with threshold ${currentThreshold}`)
        self.postMessage({ type: 'init_done' })

        // Process any buffered samples
        if (sampleBuffer.length > 0) {
          console.log(`Processing ${sampleBuffer.length} buffered sample chunks in postamble worker`)
          try {
            for (const bufferedSamples of sampleBuffer) {
              if (detector) {
                const position = detector.add_samples(bufferedSamples)
                if (position >= 0) {
                  console.log(`Postamble detected in buffered samples at position ${position}!`)
                  self.postMessage({ type: 'postamble_detected', position })
                  sampleBuffer.length = 0
                  return
                }
              }
            }
            console.log(`No postamble found in buffered samples`)
          } catch (error) {
            console.error('Error processing buffered samples:', error)
            self.postMessage({ type: 'error', error: `Buffered sample processing error: ${error}` })
            sampleBuffer.length = 0
            return
          }
        }
        sampleBuffer.length = 0
        break
      }

      case 'add_samples': {
        const { samples } = event.data as AddSamplesMessage

        // If not initialized yet, buffer the samples
        if (!isInitialized || !detector) {
          sampleBuffer.push(samples)
          console.log(`Buffering postamble samples (${sampleBuffer.length} chunks)`)
          return
        }

        try {
          const position = detector.add_samples(samples)

          // position >= 0 means postamble was detected
          if (position >= 0) {
            self.postMessage({ type: 'postamble_detected', position })
          }
        } catch (error) {
          console.error('Error during postamble detection add_samples:', error)
          self.postMessage({ type: 'error', error: `Postamble detection error: ${error}` })
        }
        break
      }

      case 'set_threshold': {
        const { threshold } = event.data as SetThresholdMessage
        currentThreshold = threshold

        if (detector) {
          try {
            detector.set_threshold(threshold)
            console.log(`Postamble detector threshold updated to ${threshold}`)
            self.postMessage({ type: 'threshold_updated' })
          } catch (e) {
            console.error('Error updating postamble detector threshold:', e)
            self.postMessage({ type: 'error', error: `Failed to update threshold: ${e}` })
          }
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
