import { createDecoder, DetectionThreshold } from '../utils/wasm'

interface InitMessage {
  type: 'init'
  preambleThreshold: number
  postambleThreshold: number
}

interface DecodeMessage {
  type: 'decode'
  samples: Float32Array
}

interface SetThresholdMessage {
  type: 'set_threshold'
  preambleThreshold?: number
  postambleThreshold?: number
}

interface ResetMessage {
  type: 'reset'
}

type WorkerMessage = InitMessage | DecodeMessage | SetThresholdMessage | ResetMessage

let decoder: any = null
let isInitialized = false
let preambleThreshold = 0.4
let postambleThreshold = 0.4

self.onmessage = async (event: MessageEvent<WorkerMessage>) => {
  try {
    const { type } = event.data

    switch (type) {
      case 'init': {
        const { preambleThreshold: pThresh, postambleThreshold: poThresh } = event.data as InitMessage
        preambleThreshold = pThresh
        postambleThreshold = poThresh

        try {
          decoder = await createDecoder({
            preambleThreshold,
            postambleThreshold,
          })
          isInitialized = true
          console.log(`Decoder worker initialized with preamble=${preambleThreshold}, postamble=${postambleThreshold}`)
          self.postMessage({ type: 'init_done' })
        } catch (error) {
          console.error('Failed to initialize decoder:', error)
          self.postMessage({ type: 'error', error: `Decoder initialization failed: ${error}` })
        }
        break
      }

      case 'decode': {
        if (!isInitialized || !decoder) {
          self.postMessage({ type: 'error', error: 'Decoder not initialized' })
          return
        }

        const { samples } = event.data as DecodeMessage

        try {
          const data = decoder.decode(samples)
          const text = new TextDecoder().decode(data)
          console.log(`Decode succeeded in worker: "${text}"`)
          self.postMessage({ type: 'decode_success', text })
        } catch (error) {
          let errorMsg = 'Decode failed'
          if (error instanceof Error) {
            errorMsg = error.message
          } else if (typeof error === 'string') {
            errorMsg = error
          }
          console.error('Decode error in worker:', errorMsg)
          self.postMessage({ type: 'decode_failed', error: errorMsg })
        }
        break
      }

      case 'set_threshold': {
        const msg = event.data as SetThresholdMessage

        if (msg.preambleThreshold !== undefined) {
          preambleThreshold = msg.preambleThreshold
        }
        if (msg.postambleThreshold !== undefined) {
          postambleThreshold = msg.postambleThreshold
        }

        if (isInitialized && decoder) {
          try {
            // Recreate decoder with new thresholds
            decoder = await createDecoder({
              preambleThreshold,
              postambleThreshold,
            })
            console.log(`Decoder thresholds updated: preamble=${preambleThreshold}, postamble=${postambleThreshold}`)
            self.postMessage({ type: 'threshold_updated' })
          } catch (error) {
            console.error('Error updating decoder thresholds:', error)
            self.postMessage({ type: 'error', error: `Failed to update thresholds: ${error}` })
          }
        }
        break
      }

      case 'reset': {
        try {
          // Recreate decoder to reset state
          decoder = await createDecoder({
            preambleThreshold,
            postambleThreshold,
          })
          console.log('Decoder reset')
          self.postMessage({ type: 'reset_done' })
        } catch (error) {
          console.error('Error resetting decoder:', error)
          self.postMessage({ type: 'error', error: `Failed to reset decoder: ${error}` })
        }
        break
      }

      default:
        self.postMessage({ type: 'error', error: `Unknown message type: ${type}` })
    }
  } catch (error) {
    self.postMessage({ type: 'error', error: String(error) })
  }
}
