import React, { useState, useRef, useEffect } from 'react'
import { flushSync } from 'react-dom'
import { useNavigate } from 'react-router-dom'
import { PreambleDetector, createFountainDecoder } from '../utils/wasm'
import { resampleAudio } from '../utils/audio'
import Status from '../components/Status'
import { getMicProcessorUrl } from '../utils/mic-processor-inline'
import {
  FOUNTAIN_BLOCK_SIZE_BYTES,
  MAX_PAYLOAD_BYTES,
  FSK_BYTES_PER_SYMBOL,
  FSK_SYMBOL_SAMPLES,
  PACKET_OVERHEAD_BYTES,
  MAX_BUFFER_SAMPLES
} from '../constants/fountain'

const TARGET_SAMPLE_RATE = 16000
const TIMEOUT_SECS = 30
const BLOCK_SIZE = FOUNTAIN_BLOCK_SIZE_BYTES
const MAX_INPUT_BYTES = MAX_PAYLOAD_BYTES

const computePacketSamples = (blockSize: number) => {
  const packetBytes = blockSize + PACKET_OVERHEAD_BYTES
  const symbolCount = Math.ceil(packetBytes / FSK_BYTES_PER_SYMBOL)
  return symbolCount * FSK_SYMBOL_SAMPLES
}

const computeMaxBufferSamples = (blockSize: number, maxInputBytes: number) => {
  const packetSamples = computePacketSamples(blockSize)
  const assumedPayload = Math.max(blockSize, maxInputBytes)
  const minPackets = Math.max(1, Math.ceil(assumedPayload / blockSize))
  const repairPackets = Math.max(2, Math.ceil(minPackets * 0.5))
  const totalPackets = minPackets + repairPackets
  const marginPackets = 1
  return (totalPackets + marginPackets) * packetSamples
}

const FountainListenPage: React.FC = () => {
  const navigate = useNavigate()

  const [isListening, setIsListening] = useState(false)
  const [isRecording, setIsRecording] = useState(false)
  const [hasRecording, setHasRecording] = useState(false)
  const [status, setStatus] = useState<string | null>(null)
  const [statusType, setStatusType] = useState<'success' | 'error' | 'info' | 'warning'>('info')
  const [elapsed, setElapsed] = useState(0)
  const [decodedText, setDecodedText] = useState<string | null>(null)
  const [isDecoding, setIsDecoding] = useState(false)
  const [sampleCount, setSampleCount] = useState(0)
  const [decodeAttempts, setDecodeAttempts] = useState(0)
  const [micVolume, setMicVolume] = useState(0)
  const [volumeGain, setVolumeGain] = useState(1)
  const [preambleThreshold, setPreambleThreshold] = useState(0.4)
  const [decodedBlocks, setDecodedBlocks] = useState(0)
  const [failedBlocks, setFailedBlocks] = useState(0)
  const [listeningMode, setListeningMode] = useState<'standard' | 'smart'>('standard')
  const [smartPacketEstimate, setSmartPacketEstimate] = useState<number>(computePacketSamples(BLOCK_SIZE))
  const [smartMaxBufferEstimate, setSmartMaxBufferEstimate] = useState<number | null>(computeMaxBufferSamples(BLOCK_SIZE, MAX_INPUT_BYTES))

  const processorRef = useRef<AudioWorkletNode | null>(null)
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const analyserRef = useRef<AnalyserNode | null>(null)
  const gainNodeRef = useRef<GainNode | null>(null)
  const resampleBufferRef = useRef<number[]>([])
  const allResampledSamplesRef = useRef<number[]>([])
  const recordedSamplesRef = useRef<number[]>([])
  const recordingResampleBufferRef = useRef<number[]>([])
  const preambleDetectedRef = useRef<boolean>(false)
  const isRecordingRef = useRef<boolean>(false)
  const startTimeRef = useRef<number>(0)
  const timerIntervalRef = useRef<number | null>(null)
  const samplesProcessedRef = useRef<number>(0)
  const streamingDecoderRef = useRef<any>(null)
  const decodeIntervalRef = useRef<number | null>(null)
  const volumeUpdateIntervalRef = useRef<number>(0)
  const workerRef = useRef<Worker | null>(null)
  const preambleWorkerRef = useRef<Worker | null>(null)
  const smartPacketSamplesRef = useRef<number>(computePacketSamples(BLOCK_SIZE))
  const smartMaxBufferSamplesRef = useRef<number | null>(computeMaxBufferSamples(BLOCK_SIZE, MAX_INPUT_BYTES))
  const decodeInFlightRef = useRef<boolean>(false)
  const listeningModeRef = useRef<'standard' | 'smart'>('standard')
  const activeBlockSizeRef = useRef<number>(BLOCK_SIZE)

  // Cleanup workers on unmount
  useEffect(() => {
    return () => {
      if (workerRef.current) {
        workerRef.current.terminate()
        workerRef.current = null
      }
      if (preambleWorkerRef.current) {
        preambleWorkerRef.current.terminate()
        preambleWorkerRef.current = null
      }
    }
  }, [])

  useEffect(() => {
    listeningModeRef.current = listeningMode
  }, [listeningMode])

  const startListening = async () => {
    // Cleanup any existing workers before creating new ones
    if (preambleWorkerRef.current) {
      preambleWorkerRef.current.terminate()
      preambleWorkerRef.current = null
    }
    if (workerRef.current) {
      workerRef.current.terminate()
      workerRef.current = null
    }

    try {
      // Initialize the fountain preamble detection worker (three-note whistle)
      const preambleWorker = new Worker(new URL('../workers/fountainPreambleDetectorWorker.ts', import.meta.url), {
        type: 'module'
      })
      preambleWorkerRef.current = preambleWorker

      // Set up preamble worker message handler that waits for init
      let isInitialized = false

      preambleWorker.onmessage = (event) => {
        const { type } = event.data

        if (type === 'init_done' && !isInitialized) {
          isInitialized = true
          console.log('Fountain preamble worker initialized (three-note whistle detector)')
        } else if (type === 'preamble_detected') {
          // Fountain preamble detected in worker, trigger recording start
          if (!preambleDetectedRef.current && isRecordingRef.current === false) {
            console.log('Fountain preamble detected from worker (three-note whistle)!')
            handlePreambleDetected()
          }
        } else if (type === 'error') {
          console.error('Fountain preamble worker error:', event.data.error)
        }
      }

      // Initialize preamble detector with the configured threshold
      preambleWorker.postMessage({ type: 'init', threshold: preambleThreshold })

      // Initialize the decoder worker
      const worker = new Worker(new URL('../workers/fountainDecoderWorker.ts', import.meta.url), {
        type: 'module'
      })
      workerRef.current = worker

      // Set up worker message handler
      worker.onmessage = (event) => {
        const { type } = event.data

        if (type === 'decode_success') {
          const { text, decodedBlocks, failedBlocks } = event.data
          decodeInFlightRef.current = false
          setDecodedText(text)
          setDecodedBlocks(decodedBlocks || 0)
          setFailedBlocks(failedBlocks || 0)
          setStatus(`Decoded successfully: "${text}"`)
          setStatusType('success')
          console.log('Decode succeeded via worker!')
          stopRecording().catch(err => console.warn('Error in stopRecording:', err))
        } else if (type === 'decode_failed') {
          const { decodedBlocks, failedBlocks } = event.data
          decodeInFlightRef.current = false
          setDecodedBlocks(decodedBlocks || 0)
          setFailedBlocks(failedBlocks || 0)
          console.log(`Decode attempt failed via worker:`, event.data.error)
        } else if (type === 'chunk_fed') {
          setSampleCount(event.data.sampleCount)
          if (typeof event.data.packetSampleEstimate === 'number') {
            smartPacketSamplesRef.current = event.data.packetSampleEstimate
            setSmartPacketEstimate(event.data.packetSampleEstimate)
          }
          if (typeof event.data.maxBufferSamples === 'number') {
            smartMaxBufferSamplesRef.current = event.data.maxBufferSamples
            setSmartMaxBufferEstimate(event.data.maxBufferSamples)
          }
        } else if (type === 'packet_ready') {
          if (typeof event.data.packetSampleEstimate === 'number') {
            smartPacketSamplesRef.current = event.data.packetSampleEstimate
            setSmartPacketEstimate(event.data.packetSampleEstimate)
          }
          if (listeningModeRef.current === 'smart') {
            tryStreamingDecode()
          }
        } else if (type === 'config_set') {
          if (typeof event.data.packetSampleEstimate === 'number') {
            smartPacketSamplesRef.current = event.data.packetSampleEstimate
            setSmartPacketEstimate(event.data.packetSampleEstimate)
          }
          if (typeof event.data.maxBufferSamples === 'number') {
            smartMaxBufferSamplesRef.current = event.data.maxBufferSamples
            setSmartMaxBufferEstimate(event.data.maxBufferSamples)
          } else {
            const fallback = computeMaxBufferSamples(BLOCK_SIZE, MAX_INPUT_BYTES)
            smartMaxBufferSamplesRef.current = fallback
            setSmartMaxBufferEstimate(fallback)
          }
        } else if (type === 'error') {
          decodeInFlightRef.current = false
          console.error('Decoder worker error:', event.data.error)
        }
      }

      const selectedBlockSize = BLOCK_SIZE
      const selectedMaxInput = MAX_INPUT_BYTES
      const packetEstimate = computePacketSamples(selectedBlockSize)
      smartPacketSamplesRef.current = packetEstimate
      setSmartPacketEstimate(packetEstimate)
      const bufferEstimate = computeMaxBufferSamples(selectedBlockSize, selectedMaxInput)
      smartMaxBufferSamplesRef.current = bufferEstimate
      setSmartMaxBufferEstimate(bufferEstimate)
      decodeInFlightRef.current = false
      activeBlockSizeRef.current = selectedBlockSize

      worker.postMessage({
        type: 'set_config',
        blockSize: selectedBlockSize,
        maxInputBytes: selectedMaxInput,
        mode: listeningMode
      })

      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
        } as any,
      })

      const audioContext = new (window.AudioContext || (window as any).webkitAudioContext)()
      audioContextRef.current = audioContext
      const source = audioContext.createMediaStreamSource(stream)

      // Create gain node for volume control
      const gainNode = audioContext.createGain()
      gainNode.gain.value = volumeGain
      gainNodeRef.current = gainNode

      // Create analyser for volume visualization
      const analyser = audioContext.createAnalyser()
      analyser.fftSize = 2048
      analyserRef.current = analyser

      if (!audioContext.audioWorklet) {
        throw new Error('AudioWorklet API is not supported')
      }

      await audioContext.audioWorklet.addModule(getMicProcessorUrl())
      const processor = new AudioWorkletNode(audioContext, 'mic-processor', {
        numberOfInputs: 1,
        numberOfOutputs: 1,
        outputChannelCount: [1],
      })

      sourceRef.current = source
      processorRef.current = processor
      streamRef.current = stream

      resampleBufferRef.current = []
      allResampledSamplesRef.current = []
      recordedSamplesRef.current = []
      recordingResampleBufferRef.current = []
      preambleDetectedRef.current = false
      isRecordingRef.current = false
      samplesProcessedRef.current = 0
      setHasRecording(false)

      source.connect(gainNode)
      gainNode.connect(analyser)
      analyser.connect(processor)
      processor.connect(audioContext.destination)

      // Start volume meter updates
      volumeUpdateIntervalRef.current = window.setInterval(() => {
        if (analyserRef.current) {
          const dataArray = new Uint8Array(analyserRef.current.frequencyBinCount)
          analyserRef.current.getByteFrequencyData(dataArray)
          const average = dataArray.reduce((a, b) => a + b) / dataArray.length
          const db = 20 * Math.log10(average / 128 + 0.0001)
          const normalizedDb = Math.max(0, Math.min(100, (db + 60) / 0.6))
          setMicVolume(normalizedDb)
        }
      }, 50)

      setIsListening(true)
      setStatus('Listening for fountain preamble (three-note whistle)...')
      setStatusType('info')
      setDecodedText(null)
      setElapsed(0)
      setSampleCount(0)
      setDecodeAttempts(0)
      setDecodedBlocks(0)
      setFailedBlocks(0)

      processor.port.onmessage = (event: MessageEvent<Float32Array>) => {
        const samples: number[] = Array.from(event.data)
        const actualSampleRate = audioContextRef.current?.sampleRate || 48000

        if (!isRecordingRef.current && !preambleDetectedRef.current) {
          resampleBufferRef.current.push(...samples)

          const chunkSize = 4096
          if (resampleBufferRef.current.length < chunkSize) {
            return
          }

          const chunk = resampleBufferRef.current.splice(0, chunkSize)
          let resampledChunk = chunk
          if (actualSampleRate !== TARGET_SAMPLE_RATE) {
            resampledChunk = resampleAudio(chunk, actualSampleRate, TARGET_SAMPLE_RATE)
          }

          allResampledSamplesRef.current.push(...resampledChunk)
          samplesProcessedRef.current += resampledChunk.length

          if (samplesProcessedRef.current > MAX_BUFFER_SAMPLES) {
            if (preambleWorkerRef.current) {
              preambleWorkerRef.current.postMessage({ type: 'clear' })
            }
            samplesProcessedRef.current = 0
            resampleBufferRef.current = []
            allResampledSamplesRef.current = []
          }

          // Send resampled chunk to preamble detection worker
          if (preambleWorkerRef.current && resampledChunk.length > 0) {
            preambleWorkerRef.current.postMessage({
              type: 'add_samples',
              samples: new Float32Array(resampledChunk)
            })
          } else if (!preambleWorkerRef.current) {
            console.warn('Preamble worker not initialized')
          }
        } else if (isRecordingRef.current) {
          recordingResampleBufferRef.current.push(...samples)

          const chunkSize = 4096
          if (recordingResampleBufferRef.current.length < chunkSize) {
            return
          }

          const chunk = recordingResampleBufferRef.current.splice(0, chunkSize)
          let resampledChunk = chunk
          if (actualSampleRate !== TARGET_SAMPLE_RATE) {
            resampledChunk = resampleAudio(chunk, actualSampleRate, TARGET_SAMPLE_RATE)
          }

          if (listeningModeRef.current === 'standard') {
            recordedSamplesRef.current.push(...resampledChunk)
          }

          // Feed chunk to worker decoder
          if (workerRef.current && resampledChunk.length > 0) {
            workerRef.current.postMessage({
              type: 'feed_chunk',
              samples: new Float32Array(resampledChunk)
            })
          }
        }
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to access microphone'
      setStatus(`Error: ${message}`)
      setStatusType('error')
    }
  }

  const handlePreambleDetected = () => {
    preambleDetectedRef.current = true
    isRecordingRef.current = true
    startTimeRef.current = Date.now()

    const initialSamples = allResampledSamplesRef.current
    const initialLength = initialSamples.length
    if (listeningModeRef.current === 'standard') {
      recordedSamplesRef.current = initialSamples
    } else {
      recordedSamplesRef.current = []
    }
    allResampledSamplesRef.current = []
    resampleBufferRef.current = []

    // Force immediate UI update when fountain preamble is detected
    flushSync(() => {
      setIsRecording(true)
      setStatus('Fountain preamble detected (three-note whistle)! Starting streaming decode...')
      setStatusType('success')
      setSampleCount(initialLength)
      setDecodeAttempts(0)
    })

    // Feed initial samples to decoder worker (everything before preamble)
    if (workerRef.current && initialSamples.length > 0) {
      workerRef.current.postMessage({
        type: 'feed_chunk',
        samples: new Float32Array(initialSamples)
      })
    }

    // Timer for UI updates; enforce timeout only in standard mode
    timerIntervalRef.current = window.setInterval(() => {
      const elapsedSecs = (Date.now() - startTimeRef.current) / 1000
      setElapsed(elapsedSecs)

      if (listeningModeRef.current === 'standard' && elapsedSecs >= TIMEOUT_SECS) {
        stopRecording('Timeout reached (30 seconds)').catch(err => console.warn('Error in stopRecording:', err))
      }
    }, 100)

    // Periodic decode attempts (every 2 seconds) for standard mode only
    if (listeningModeRef.current === 'standard') {
      decodeIntervalRef.current = window.setInterval(() => {
        tryStreamingDecode()
      }, 2000)
    } else {
      decodeIntervalRef.current = null
      decodeInFlightRef.current = false
    }

    // Clear preamble detector since we detected it
    if (preambleWorkerRef.current) {
      preambleWorkerRef.current.postMessage({ type: 'clear' })
    }
  }

  const tryStreamingDecode = () => {
    if (!workerRef.current || !isRecordingRef.current) {
      return
    }

    if (decodeInFlightRef.current) {
      return
    }
    decodeInFlightRef.current = true

    setDecodeAttempts(prev => {
      const next = prev + 1
      console.log(`Decode attempt #${next}`)
      return next
    })

    // Send decode attempt to worker (it will respond asynchronously via onmessage)
    workerRef.current.postMessage({ type: 'try_decode' })
  }

  const stopListening = async () => {
    await cleanup()
    setIsListening(false)
    setIsRecording(false)
    setStatus('Stopped listening')
    setStatusType('info')
    setMicVolume(0)
  }

  const stopRecording = async (message?: string) => {
    // Flush any remaining samples in the recording buffer
    if (recordingResampleBufferRef.current.length > 0) {
      const actualSampleRate = audioContextRef.current?.sampleRate || 48000
      let resampledChunk = recordingResampleBufferRef.current
      if (actualSampleRate !== TARGET_SAMPLE_RATE) {
        resampledChunk = resampleAudio(recordingResampleBufferRef.current, actualSampleRate, TARGET_SAMPLE_RATE)
      }
      if (listeningModeRef.current === 'standard') {
        recordedSamplesRef.current.push(...resampledChunk)
      }
      recordingResampleBufferRef.current = []
    }

    await cleanup()
    isRecordingRef.current = false
    setIsRecording(false)
    setIsListening(false)

    const hasRecordedAudio = recordedSamplesRef.current.length > 0
    setHasRecording(listeningModeRef.current === 'standard' ? hasRecordedAudio : false)
    console.log('Stop recording - samples:', recordedSamplesRef.current.length, 'hasRecording:', hasRecordedAudio)

    if (message) {
      setStatus(message)
      setStatusType('success')
    }
  }

  const cleanup = async () => {
    if (processorRef.current) {
      processorRef.current.port.onmessage = null
      processorRef.current.disconnect()
      processorRef.current = null
    }

    if (sourceRef.current) {
      sourceRef.current.disconnect()
      sourceRef.current = null
    }

    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop())
      streamRef.current = null
    }

    if (audioContextRef.current) {
      try {
        await audioContextRef.current.close()
      } catch (error) {
        console.warn('Error closing AudioContext:', error)
      }
      audioContextRef.current = null
    }

    if (timerIntervalRef.current) {
      clearInterval(timerIntervalRef.current)
      timerIntervalRef.current = null
    }

    if (decodeIntervalRef.current) {
      clearInterval(decodeIntervalRef.current)
      decodeIntervalRef.current = null
    }

    if (volumeUpdateIntervalRef.current) {
      clearInterval(volumeUpdateIntervalRef.current)
      volumeUpdateIntervalRef.current = 0
    }

    if (streamingDecoderRef.current) {
      streamingDecoderRef.current = null
    }

    if (workerRef.current) {
      workerRef.current.terminate()
      workerRef.current = null
    }

    if (preambleWorkerRef.current) {
      preambleWorkerRef.current.terminate()
      preambleWorkerRef.current = null
    }

    decodeInFlightRef.current = false
  }

  const decodeFountainAudio = async () => {
    if (recordedSamplesRef.current.length === 0) {
      setStatus('No audio recorded')
      setStatusType('error')
      return
    }

    try {
      setIsDecoding(true)
      setStatus('Decoding fountain stream...')
      setStatusType('info')

      const samples = new Float32Array(recordedSamplesRef.current)
      console.log('Decoding with:', {
        sampleCount: samples.length,
        timeoutSecs: TIMEOUT_SECS,
        blockSize: activeBlockSizeRef.current,
        sampleRate: TARGET_SAMPLE_RATE,
        firstSamples: Array.from(samples.slice(0, 10)),
        hasNaN: samples.some(s => isNaN(s)),
        hasInfinity: samples.some(s => !isFinite(s))
      })

      const decoder = await createFountainDecoder(preambleThreshold)
      const data = decoder.decode_fountain(
        samples,
        TIMEOUT_SECS,
        activeBlockSizeRef.current
      )
      const text = new TextDecoder().decode(data)

      // Capture block statistics from decoder
      const decodedBlocksCount = decoder.get_decoded_blocks()
      const failedBlocksCount = decoder.get_failed_blocks()

      setDecodedText(text)
      setDecodedBlocks(decodedBlocksCount)
      setFailedBlocks(failedBlocksCount)
      setStatus(`Decoded successfully: "${text}"`)
      setStatusType('success')
    } catch (error) {
      let message = 'Decode failed'
      if (error instanceof Error) {
        message = error.message
      } else if (typeof error === 'string') {
        message = error
      }
      console.error('Decode error:', error)
      setStatus(message)
      setStatusType('error')
    } finally {
      setIsDecoding(false)
    }
  }

  const downloadWavFile = () => {
    if (recordedSamplesRef.current.length === 0) {
      setStatus('No audio recorded to download')
      setStatusType('error')
      return
    }

    const sampleRate = TARGET_SAMPLE_RATE
    const numChannels = 1
    const bitsPerSample = 16
    const samples = recordedSamplesRef.current

    const dataLength = samples.length * 2
    const buffer = new ArrayBuffer(44 + dataLength)
    const view = new DataView(buffer)

    const writeString = (offset: number, str: string) => {
      for (let i = 0; i < str.length; i++) {
        view.setUint8(offset + i, str.charCodeAt(i))
      }
    }

    writeString(0, 'RIFF')
    view.setUint32(4, 36 + dataLength, true)
    writeString(8, 'WAVE')
    writeString(12, 'fmt ')
    view.setUint32(16, 16, true)
    view.setUint16(20, 1, true)
    view.setUint16(22, numChannels, true)
    view.setUint32(24, sampleRate, true)
    view.setUint32(28, sampleRate * numChannels * (bitsPerSample / 8), true)
    view.setUint16(32, numChannels * (bitsPerSample / 8), true)
    view.setUint16(34, bitsPerSample, true)
    writeString(36, 'data')
    view.setUint32(40, dataLength, true)

    let offset = 44
    for (let i = 0; i < samples.length; i++) {
      const sample = Math.max(-1, Math.min(1, samples[i]))
      const int16 = sample < 0 ? sample * 0x8000 : sample * 0x7FFF
      view.setInt16(offset, int16, true)
      offset += 2
    }

    const blob = new Blob([buffer], { type: 'audio/wav' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `fountain-recorded-${Date.now()}.wav`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)

    setStatus('Recording downloaded')
    setStatusType('success')
  }

  const handleFileUpload = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    if (!file) return

    let audioContext: AudioContext | null = null

    try {
      setStatus('Loading WAV file...')
      setStatusType('info')

      const arrayBuffer = await file.arrayBuffer()
      audioContext = new (window.AudioContext || (window as any).webkitAudioContext)()
      const audioBuffer = await audioContext.decodeAudioData(arrayBuffer)

      // Extract samples from first channel
      const samples = audioBuffer.getChannelData(0)

      // Resample if needed
      let finalSamples: number[] = Array.from(samples)
      if (audioBuffer.sampleRate !== TARGET_SAMPLE_RATE) {
        finalSamples = resampleAudio(
          Array.from(samples),
          audioBuffer.sampleRate,
          TARGET_SAMPLE_RATE
        )
      }

      recordedSamplesRef.current = finalSamples
      setHasRecording(true)
      setDecodedText(null)
      setStatus(`Loaded ${finalSamples.length} samples from ${file.name}`)
      setStatusType('success')

      console.log('Uploaded file:', {
        name: file.name,
        originalSampleRate: audioBuffer.sampleRate,
        targetSampleRate: TARGET_SAMPLE_RATE,
        originalSamples: samples.length,
        resampledSamples: finalSamples.length
      })
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to load WAV file'
      setStatus(`Error: ${message}`)
      setStatusType('error')
      console.error('File upload error:', error)
    } finally {
      if (audioContext && typeof audioContext.close === 'function') {
        try {
          await audioContext.close()
        } catch (error) {
          console.warn('Error closing AudioContext:', error)
        }
      }
    }
  }

  const resetAndListenAgain = () => {
    preambleDetectedRef.current = false
    isRecordingRef.current = false
    recordedSamplesRef.current = []
    recordingResampleBufferRef.current = []
    streamingDecoderRef.current = null
    if (workerRef.current) {
      workerRef.current.terminate()
      workerRef.current = null
    }
    if (preambleWorkerRef.current) {
      preambleWorkerRef.current.terminate()
      preambleWorkerRef.current = null
    }
    setDecodedText(null)
    setElapsed(0)
    setStatus(null)
    setHasRecording(false)
    setSampleCount(0)
    setDecodeAttempts(0)
    startListening()
  }

  const progressPercent = Math.min(100, (elapsed / TIMEOUT_SECS) * 100)
  const isSmartMode = listeningMode === 'smart'
  const displayPacketSamples = smartPacketEstimate
  const displayMaxBufferSamples = smartMaxBufferEstimate ?? (isSmartMode
    ? computeMaxBufferSamples(BLOCK_SIZE, MAX_INPUT_BYTES)
    : null)

  return (
    <div className="container">
      <div className="text-center mb-5">
        <h1>Fountain Code Listener (Streaming Mode)</h1>
        <p>Detects preamble, then continuously attempts to decode until successful or 30s timeout</p>
      </div>

      <div className="card" style={{ maxWidth: '600px', margin: '0 auto' }}>
        <h2>Listening Controls</h2>

        <div className="mt-4">
          <label><strong>Listening Mode</strong></label>
          <div style={{ display: 'flex', gap: '1rem', marginTop: '0.5rem', flexWrap: 'wrap' }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', fontSize: '0.95rem' }}>
              <input
                type="radio"
                name="listening-mode"
                value="standard"
                checked={listeningMode === 'standard'}
                onChange={() => {
                  setListeningMode('standard')
                  activeBlockSizeRef.current = BLOCK_SIZE
                }}
                disabled={isListening || isRecording}
              />
              Standard (record & replay)
            </label>
            <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', fontSize: '0.95rem' }}>
              <input
                type="radio"
                name="listening-mode"
                value="smart"
                checked={isSmartMode}
                onChange={() => {
                  setListeningMode('smart')
                  activeBlockSizeRef.current = BLOCK_SIZE
                }}
                disabled={isListening || isRecording}
              />
              Smart streaming (live decode)
            </label>
          </div>
          <small style={{ display: 'block', marginTop: '0.5rem', color: '#64748b' }}>
            {isSmartMode
              ? 'Smart streaming keeps everything in memory—no WAV file is saved.'
              : 'Standard mode stores the captured audio so you can decode or download it later.'}
          </small>
        </div>

        {isSmartMode && (
          <div className="mt-4" style={{ background: '#f8fafc', padding: '1rem', borderRadius: '0.5rem', fontSize: '0.9rem', color: '#334155' }}>
            <p style={{ marginBottom: '0.75rem' }}>
              Smart streaming will attempt fountain decodes as soon as it receives enough samples for a full block.
              Both sides use a fixed block size of <strong>{BLOCK_SIZE} bytes</strong> and a max payload of <strong>{MAX_INPUT_BYTES} bytes</strong>.
            </p>
            <div style={{ fontSize: '0.85rem', color: '#475569', background: '#eef2ff', padding: '0.75rem', borderRadius: '0.5rem' }}>
              <div>≈ Samples per fountain packet: {displayPacketSamples.toLocaleString()}</div>
              {displayMaxBufferSamples !== null && (
                <div>≈ Max buffered samples before trimming: {displayMaxBufferSamples.toLocaleString()}</div>
              )}
            </div>
          </div>
        )}

        <div className="mt-4">
          <button
            onClick={startListening}
            disabled={isListening}
            className="btn-primary w-full"
          >
            {isSmartMode ? 'Start Smart Listening' : 'Start Listening'}
          </button>
        </div>

        {!isSmartMode && (
          <div className="mt-4" style={{
            borderTop: '1px solid #e2e8f0',
            paddingTop: '1rem',
            textAlign: 'center'
          }}>
            <p style={{ marginBottom: '0.5rem', color: '#64748b' }}>OR</p>
            <label
              htmlFor="wav-upload"
              className="btn-secondary w-full"
              style={{ display: 'inline-block', cursor: 'pointer' }}
            >
              Upload WAV File
            </label>
            <input
              id="wav-upload"
              type="file"
              accept=".wav,audio/wav"
              onChange={handleFileUpload}
              disabled={isListening || isRecording}
              style={{ display: 'none' }}
            />
          </div>
        )}

        {isListening && !isRecording && (
          <div className="mt-4">
            <button onClick={stopListening} className="btn-secondary w-full">
              Stop Listening
            </button>
          </div>
        )}

        {isRecording && (
          <div className="mt-4">
            <button onClick={() => stopRecording('Recording stopped manually').catch(err => console.warn('Error in stopRecording:', err))} className="btn-secondary w-full">
              Stop Recording
            </button>
          </div>
        )}

        {status && <Status message={status} type={statusType} />}

        {isRecording && (
          <div className="mt-4">
            <div style={{ marginBottom: '0.5rem', display: 'flex', justifyContent: 'space-between' }}>
              <span><strong>{isSmartMode ? 'Streaming time:' : 'Progress:'}</strong></span>
              <span>{isSmartMode ? `${elapsed.toFixed(1)}s` : `${elapsed.toFixed(1)}s / ${TIMEOUT_SECS}s`}</span>
            </div>
            {!isSmartMode ? (
              <div style={{
                width: '100%',
                height: '8px',
                background: '#e2e8f0',
                borderRadius: '4px',
                overflow: 'hidden',
                marginBottom: '1rem'
              }}>
                <div style={{
                  width: `${progressPercent}%`,
                  height: '100%',
                  background: '#4299e1',
                  transition: 'width 0.1s linear'
                }} />
              </div>
            ) : (
              <div style={{
                background: '#f8fafc',
                border: '1px solid #cbd5e0',
                borderRadius: '0.5rem',
                padding: '0.75rem',
                marginBottom: '1rem',
                color: '#475569',
                fontSize: '0.9rem'
              }}>
                Smart mode runs continuously until a decode succeeds or you stop listening.
              </div>
            )}
            <div style={{ fontSize: '0.9rem', color: '#64748b' }}>
              <div>Samples accumulated: {sampleCount.toLocaleString()}</div>
              <div>Decode attempts: {decodeAttempts}</div>
              {isSmartMode && (
                <div style={{ marginTop: '0.5rem' }}>
                  <div>Packet sample estimate: {displayPacketSamples.toLocaleString()}</div>
                  {displayMaxBufferSamples !== null && (
                    <div>Buffer cap before trimming: {displayMaxBufferSamples.toLocaleString()}</div>
                  )}
                </div>
              )}
              <div style={{ marginTop: '0.5rem', paddingTop: '0.5rem', borderTop: '1px solid #cbd5e0' }}>
                <div>Successfully decoded blocks: {decodedBlocks}</div>
                <div>Failed blocks (CRC): {failedBlocks}</div>
              </div>
            </div>
          </div>
        )}

        {!isRecording && (decodedBlocks > 0 || failedBlocks > 0) && (
          <div className="mt-4" style={{ fontSize: '0.9rem', color: '#64748b', padding: '0.75rem', background: '#f0fdf4', borderRadius: '0.375rem', borderLeft: '3px solid #22c55e' }}>
            <div><strong>Decode Statistics:</strong></div>
            <div style={{ marginTop: '0.5rem' }}>
              <div>Successfully decoded blocks: {decodedBlocks}</div>
              <div>Failed blocks (CRC): {failedBlocks}</div>
            </div>
          </div>
        )}

        <div className="mt-4" style={{ padding: '1rem', background: '#f7fafc', borderRadius: '0.5rem', fontSize: '0.9rem' }}>
          <p><strong>Configuration:</strong></p>
          <ul style={{ marginTop: '0.5rem', paddingLeft: '1.5rem', marginBottom: '1rem' }}>
            <li>Duration: {TIMEOUT_SECS} seconds</li>
            <li>Mode: {isSmartMode ? 'Smart streaming (live decode)' : 'Standard recording'}</li>
            <li>Block size: {BLOCK_SIZE} bytes</li>
            {isSmartMode && <li>Max payload: {MAX_INPUT_BYTES} bytes</li>}
          </ul>

          <div style={{ marginTop: '1rem' }}>
            <label><strong>Microphone Volume</strong></label>
            <div className="flex items-center gap-3 mt-2">
              <input
                type="range"
                min="0.5"
                max="10"
                step="0.1"
                value={volumeGain}
                onChange={(e) => {
                  const newGain = parseFloat(e.target.value)
                  setVolumeGain(newGain)
                  if (gainNodeRef.current) {
                    gainNodeRef.current.gain.value = newGain
                  }
                }}
                disabled={isListening}
              />
              <span>{volumeGain.toFixed(1)}x</span>
            </div>
            <small>Amplify microphone input (0.5x to 10x). Recommended: 1.0x</small>
          </div>

          <div style={{ marginTop: '1rem' }}>
            <label><strong>Preamble Detection Threshold</strong></label>
            <div className="flex items-center gap-3 mt-2">
              <input
                type="range"
                min="0.1"
                max="0.9"
                step="0.05"
                value={preambleThreshold}
                onChange={(e) => setPreambleThreshold(parseFloat(e.target.value))}
                disabled={isListening}
              />
              <span>{preambleThreshold.toFixed(2)}</span>
            </div>
            <small>Lower values = more sensitive. Default: 0.4</small>
          </div>

          <div style={{ marginTop: '1rem' }}>
            <label><strong>Input Level</strong></label>
            <div style={{ background: '#f7fafc', padding: '1rem', borderRadius: '0.5rem', marginTop: '0.5rem' }}>
              <div style={{ display: 'flex', gap: '0.5rem', height: '20px', background: '#e2e8f0', borderRadius: '4px', overflow: 'hidden' }}>
                <div style={{
                  background: 'linear-gradient(90deg, #4ade80, #facc15, #ef4444)',
                  height: '100%',
                  width: `${micVolume}%`,
                  transition: 'width 0.05s linear'
                }}></div>
              </div>
              <div style={{ marginTop: '0.5rem', fontSize: '0.85rem', color: '#666' }}>
                Peak: {(micVolume * 0.6 - 60).toFixed(1)} dB
              </div>
            </div>
          </div>

        </div>
      </div>

      {!isListening && !isRecording && hasRecording && (
        <div className="card" style={{ maxWidth: '600px', margin: '2rem auto 0' }}>
          <h2>Recording Complete</h2>

          <div className="mt-4">
            <button
              onClick={decodeFountainAudio}
              disabled={isDecoding}
              className="btn-primary w-full"
            >
              {isDecoding ? 'Decoding...' : 'Decode Recording'}
            </button>
          </div>

          <div className="mt-4">
            <button
              onClick={downloadWavFile}
              className="btn-secondary w-full"
            >
              Download Recording (.wav)
            </button>
          </div>

          {decodedText && (
            <>
              <h3 style={{ marginTop: '2rem' }}>Decoded Result</h3>
              <div style={{
                background: '#f0f9ff',
                padding: '0.75rem 1rem',
                borderRadius: '0.5rem',
                fontSize: '0.9rem',
                color: '#0369a1',
                marginTop: '1rem',
                marginBottom: '1rem'
              }}>
                Decoded in {decodeAttempts} attempt{decodeAttempts !== 1 ? 's' : ''} ({(decodeAttempts * 2).toFixed(0)}s)
              </div>
              <div style={{
                background: '#f7fafc',
                padding: '1rem',
                borderRadius: '0.5rem',
                wordBreak: 'break-word',
                fontFamily: 'monospace',
                minHeight: '80px',
                marginTop: '0.5rem'
              }}>
                {decodedText}
              </div>
            </>
          )}

          <button
            onClick={resetAndListenAgain}
            className="btn-secondary w-full mt-4"
          >
            Listen Again
          </button>
        </div>
      )}

      <div className="text-center mt-4">
        <button onClick={() => navigate('/')} className="btn-secondary">
          ← Back to Home
        </button>
      </div>
    </div>
  )
}

export default FountainListenPage
