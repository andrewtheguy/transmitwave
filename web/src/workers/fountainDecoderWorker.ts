import { createFountainDecoder, initWasm } from '../utils/wasm'

interface FeedChunkMessage {
  type: 'feed_chunk'
  samples: Float32Array
}

interface SetBlockSizeMessage {
  type: 'set_block_size'
  blockSize: number
}

interface TryDecodeMessage {
  type: 'try_decode'
}

interface ResetMessage {
  type: 'reset'
}

type WorkerMessage = FeedChunkMessage | SetBlockSizeMessage | TryDecodeMessage | ResetMessage

let sampleBuffer: Float32Array[] = []
let blockSize = 16
let wasmInitialized = false
const MAX_BUFFER_SAMPLES = 80000 // ~5 seconds at 16kHz

// No eager initialization - initialize on first use to avoid race conditions

async function createNewDecoder() {
  // Ensure WASM is initialized
  if (!wasmInitialized) {
    try {
      await initWasm()
      wasmInitialized = true
    } catch (error) {
      console.error('Failed to initialize WASM in decoder worker:', error)
      throw error
    }
  }

  const decoder = await createFountainDecoder()
  decoder.set_block_size(blockSize)
  return decoder
}

self.onmessage = async (event: MessageEvent<WorkerMessage>) => {
  try {
    const { type } = event.data

    switch (type) {
      case 'feed_chunk': {
        const { samples } = event.data as FeedChunkMessage
        sampleBuffer.push(samples)

        const totalSamples = sampleBuffer.reduce((sum, s) => sum + s.length, 0)
        // // Prevent unbounded buffer growth
        // if (totalSamples > MAX_BUFFER_SAMPLES) {
        //   console.warn(`Buffer overflow: ${totalSamples} > ${MAX_BUFFER_SAMPLES}, clearing buffer`)
        //   sampleBuffer.length = 0
        // }

        self.postMessage({ type: 'chunk_fed', sampleCount: totalSamples })
        break
      }

      case 'set_block_size': {
        const { blockSize: newBlockSize } = event.data as SetBlockSizeMessage
        blockSize = newBlockSize
        self.postMessage({ type: 'block_size_set' })
        break
      }

      case 'try_decode': {
        try {
          // Create a fresh decoder for each attempt to avoid aliasing issues
          const decoder = await createNewDecoder()

          // Feed all buffered samples to the decoder
          for (const chunk of sampleBuffer) {
            decoder.feed_chunk(chunk)
          }

          // Try to decode
          const data = decoder.try_decode()
          const text = new TextDecoder().decode(data)
          const totalSamples = sampleBuffer.reduce((sum, s) => sum + s.length, 0)
          const decodedBlocks = decoder.get_decoded_blocks()
          const failedBlocks = decoder.get_failed_blocks()
          self.postMessage({
            type: 'decode_success',
            text,
            sampleCount: totalSamples,
            decodedBlocks,
            failedBlocks
          })
        } catch (error) {
          const totalSamples = sampleBuffer.reduce((sum, s) => sum + s.length, 0)
          // Still get stats even on decode failure
          try {
            const decoder = await createNewDecoder()
            for (const chunk of sampleBuffer) {
              decoder.feed_chunk(chunk)
            }
            const decodedBlocks = decoder.get_decoded_blocks()
            const failedBlocks = decoder.get_failed_blocks()
            self.postMessage({
              type: 'decode_failed',
              error: String(error),
              sampleCount: totalSamples,
              decodedBlocks,
              failedBlocks
            })
          } catch {
            self.postMessage({
              type: 'decode_failed',
              error: String(error),
              sampleCount: totalSamples,
              decodedBlocks: 0,
              failedBlocks: 0
            })
          }
        }
        break
      }

      case 'reset': {
        sampleBuffer.length = 0
        self.postMessage({ type: 'reset_done' })
        break
      }

      default:
        self.postMessage({ type: 'error', error: `Unknown message type: ${type}` })
    }
  } catch (error) {
    self.postMessage({ type: 'error', error: String(error) })
  }
}
