import { createDecoder, createDecoderDtmf, DetectionThreshold } from '../utils/wasm'

interface InitMessage {
  type: 'init'
  preambleThreshold: number
  postambleThreshold: number
  mode?: 'fsk' | 'dtmf' // Default to 'fsk' for backward compatibility
}

interface DecodeMessage {
  type: 'decode'
  samples: Float32Array
}

interface DecodeWithoutSyncMessage {
  type: 'decode_without_sync'
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

type WorkerMessage = InitMessage | DecodeMessage | DecodeWithoutSyncMessage | SetThresholdMessage | ResetMessage

let decoder: any = null
let isInitialized = false
let preambleThreshold = 0.4
let postambleThreshold = 0.4
let mode: 'fsk' | 'dtmf' = 'fsk' // Default to FSK

self.onmessage = async (event: MessageEvent<WorkerMessage>) => {
  try {
    const { type } = event.data

    switch (type) {
      case 'init': {
        const { preambleThreshold: pThresh, postambleThreshold: poThresh, mode: msgMode } = event.data as InitMessage
        preambleThreshold = pThresh
        postambleThreshold = poThresh
        mode = msgMode || 'fsk' // Default to FSK if not specified

        try {
          if (mode === 'dtmf') {
            decoder = await createDecoderDtmf({
              preambleThreshold,
              postambleThreshold,
            })
          } else {
            decoder = await createDecoder({
              preambleThreshold,
              postambleThreshold,
            })
          }
          isInitialized = true
          console.log(`Decoder worker initialized with mode=${mode}, preamble=${preambleThreshold}, postamble=${postambleThreshold}`)
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

      case 'decode_without_sync': {
        if (!isInitialized || !decoder) {
          self.postMessage({ type: 'error', error: 'Decoder not initialized' })
          return
        }

        const { samples } = event.data as DecodeWithoutSyncMessage

        try {
          const data = decoder.decode_without_preamble_postamble(samples)
          const text = new TextDecoder().decode(data)
          console.log(`Decode without sync succeeded in worker: "${text}"`)
          self.postMessage({ type: 'decode_success', text })
        } catch (error) {
          let errorMsg = 'Decode without sync failed'
          if (error instanceof Error) {
            errorMsg = error.message
          } else if (typeof error === 'string') {
            errorMsg = error
          }
          console.error('Decode without sync error in worker:', errorMsg)
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
            // Recreate decoder with new thresholds using current mode
            if (mode === 'dtmf') {
              decoder = await createDecoderDtmf({
                preambleThreshold,
                postambleThreshold,
              })
            } else {
              decoder = await createDecoder({
                preambleThreshold,
                postambleThreshold,
              })
            }
            console.log(`Decoder thresholds updated: mode=${mode}, preamble=${preambleThreshold}, postamble=${postambleThreshold}`)
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
          // Recreate decoder to reset state using current mode
          if (mode === 'dtmf') {
            decoder = await createDecoderDtmf({
              preambleThreshold,
              postambleThreshold,
            })
          } else {
            decoder = await createDecoder({
              preambleThreshold,
              postambleThreshold,
            })
          }
          console.log(`Decoder reset with mode=${mode}`)
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
