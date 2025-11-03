import { createFountainDecoder, initWasm } from '../utils/wasm'
import { FOUNTAIN_BLOCK_SIZE_BYTES, FOUNTAIN_MAX_PAYLOAD_BYTES } from '../constants/fountain'

interface InitMessage {
  type: 'init'
}

interface FeedChunkMessage {
  type: 'feed_chunk'
  samples: Float32Array
}

interface SetConfigMessage {
  type: 'set_config'
  blockSize: number
  maxInputBytes?: number
  mode?: 'standard' | 'smart'
}

interface TryDecodeMessage {
  type: 'try_decode'
}

interface ResetMessage {
  type: 'reset'
}

type WorkerMessage =
  | InitMessage
  | FeedChunkMessage
  | SetConfigMessage
  | TryDecodeMessage
  | ResetMessage

let sampleBuffer: Float32Array[] = []
let blockSize = FOUNTAIN_BLOCK_SIZE_BYTES
let wasmInitialized = false
let maxInputBytes = FOUNTAIN_MAX_PAYLOAD_BYTES
let totalSamples = 0
let samplesPerPacket = 0
let maxBufferSamples = 0
let streamingMode: 'standard' | 'smart' = 'standard'
let samplesSinceLastHint = 0

const FSK_BYTES_PER_SYMBOL = 3
const FSK_SYMBOL_SAMPLES = 3072
const PACKET_OVERHEAD_BYTES = 14

function recomputeLimits() {
  samplesPerPacket = computePacketSamples(blockSize)
  maxBufferSamples = computeMaxBufferSamples(blockSize, maxInputBytes)
}

function computePacketSamples(currentBlockSize: number): number {
  const symbolBytes = currentBlockSize + PACKET_OVERHEAD_BYTES
  const symbolCount = Math.ceil(symbolBytes / FSK_BYTES_PER_SYMBOL)
  return symbolCount * FSK_SYMBOL_SAMPLES
}

function computeMaxBufferSamples(currentBlockSize: number, currentMaxInput: number): number {
  const packetSamples = computePacketSamples(currentBlockSize)
  const assumedPayload = Math.max(currentBlockSize, currentMaxInput)
  const minPackets = Math.max(1, Math.ceil(assumedPayload / currentBlockSize))
  const repairPackets = Math.max(2, Math.ceil(minPackets * 0.5))
  const totalPackets = minPackets + repairPackets
  const marginPackets = 1
  return (totalPackets + marginPackets) * packetSamples
}

function trimBufferIfNeeded() {
  if (streamingMode !== 'smart' || totalSamples <= maxBufferSamples || maxBufferSamples === 0) {
    return
  }

  while (sampleBuffer.length > 0 && totalSamples > maxBufferSamples) {
    const removed = sampleBuffer.shift()
    if (!removed) {
      break
    }
    totalSamples -= removed.length
  }
}

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
      case 'init': {
        console.log(`Fountain decoder worker initialized`)
        self.postMessage({ type: 'init_done' })
        break
      }

      case 'feed_chunk': {
        const { samples } = event.data as FeedChunkMessage
        sampleBuffer.push(samples)
        totalSamples += samples.length
        trimBufferIfNeeded()
        if (streamingMode === 'smart' && samplesPerPacket > 0) {
          samplesSinceLastHint += samples.length
          while (samplesSinceLastHint >= samplesPerPacket) {
            samplesSinceLastHint -= samplesPerPacket
            self.postMessage({
              type: 'packet_ready',
              sampleCount: totalSamples,
              packetSampleEstimate: samplesPerPacket
            })
          }
        }

        self.postMessage({
          type: 'chunk_fed',
          sampleCount: totalSamples,
          packetSampleEstimate: samplesPerPacket,
          maxBufferSamples
        })
        break
      }

      case 'set_config': {
        const { blockSize: newBlockSize, maxInputBytes: newMaxInput, mode } = event.data as SetConfigMessage
        if (typeof newBlockSize === 'number' && Number.isFinite(newBlockSize) && newBlockSize > 0) {
          blockSize = Math.floor(newBlockSize)
        }
        if (typeof newMaxInput === 'number' && Number.isFinite(newMaxInput) && newMaxInput > 0) {
          maxInputBytes = Math.floor(newMaxInput)
        }
        if (mode === 'smart' || mode === 'standard') {
          streamingMode = mode
        }
        recomputeLimits()
        samplesSinceLastHint = 0
        self.postMessage({
          type: 'config_set',
          blockSize,
          maxInputBytes,
          packetSampleEstimate: samplesPerPacket,
          maxBufferSamples
        })
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
          const decodedBlocks = decoder.get_decoded_blocks()
          const failedBlocks = decoder.get_failed_blocks()
          const consumedSamples = totalSamples
          sampleBuffer.length = 0
          totalSamples = 0
          samplesSinceLastHint = 0
          self.postMessage({
            type: 'decode_success',
            text,
            sampleCount: consumedSamples,
            decodedBlocks,
            failedBlocks
          })
        } catch (error) {
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
        totalSamples = 0
        samplesSinceLastHint = 0
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

// Initialize computed values with defaults
recomputeLimits()
