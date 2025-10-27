import { createFountainDecoder } from '../utils/wasm'

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

let decoder: any = null
let blockSize = 64

async function initDecoder() {
  if (!decoder) {
    decoder = await createFountainDecoder()
    decoder.set_block_size(blockSize)
  }
}

self.onmessage = async (event: MessageEvent<WorkerMessage>) => {
  try {
    const { type } = event.data

    switch (type) {
      case 'feed_chunk': {
        await initDecoder()
        const { samples } = event.data as FeedChunkMessage
        decoder.feed_chunk(samples)
        const sampleCount = decoder.get_sample_count()
        self.postMessage({ type: 'chunk_fed', sampleCount })
        break
      }

      case 'set_block_size': {
        await initDecoder()
        const { blockSize: newBlockSize } = event.data as SetBlockSizeMessage
        blockSize = newBlockSize
        decoder.set_block_size(blockSize)
        self.postMessage({ type: 'block_size_set' })
        break
      }

      case 'try_decode': {
        await initDecoder()
        try {
          const data = decoder.try_decode()
          const text = new TextDecoder().decode(data)
          self.postMessage({ type: 'decode_success', text, sampleCount: decoder.get_sample_count() })
        } catch (error) {
          const sampleCount = decoder.get_sample_count()
          self.postMessage({ type: 'decode_failed', error: String(error), sampleCount })
        }
        break
      }

      case 'reset': {
        if (decoder) {
          decoder.reset()
        }
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
