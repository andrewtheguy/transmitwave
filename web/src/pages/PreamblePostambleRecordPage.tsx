import React, { useState, useRef, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { resampleAudio } from '../utils/audio'
import Status from '../components/Status'
import { getMicProcessorUrl } from '../utils/mic-processor-inline'

const MAX_RECORDING_DURATION = 30
const TARGET_SAMPLE_RATE = 16000
const MAX_BUFFER_SAMPLES = 80000 // Listening phase buffer cap (~5 seconds at 16kHz)
const MAX_RECORDING_SAMPLES = 480000 // Recording phase buffer cap (~30 seconds at 16kHz)
const PREAMBLE_DURATION_MS = 250
const PREAMBLE_SAMPLES = (TARGET_SAMPLE_RATE * PREAMBLE_DURATION_MS) / 1000 // 4000
const PRE_ROLL_MS = 100
const PRE_ROLL_SAMPLES = (TARGET_SAMPLE_RATE * PRE_ROLL_MS) / 1000 // keep ~0.1s before preamble for safety
const AUTO_GAIN_MIN = 0.3
const AUTO_GAIN_MAX = 12.0
const AUTO_GAIN_SMOOTHING = 0.25
const AUTO_GAIN_TOLERANCE = 0.08 // 8% error window before adjusting again

const PreamblePostambleRecordPage: React.FC = () => {
  const navigate = useNavigate()

  // Helper function to calculate RMS (Root Mean Square) amplitude
  const calculateRMS = (samples: number[]): number => {
    if (samples.length === 0) return 0
    const sumSquares = samples.reduce((sum, sample) => sum + sample * sample, 0)
    return Math.sqrt(sumSquares / samples.length)
  }

  const applyAutoGain = (sample: number, gain: number): number => {
    const gainedSample = sample * gain
    if (Math.abs(gainedSample) > 1.0) {
      return Math.sign(gainedSample) * (1.0 - Math.exp(-Math.abs(gainedSample)))
    }
    return gainedSample
  }

  const clampGain = (gain: number) => Math.max(AUTO_GAIN_MIN, Math.min(AUTO_GAIN_MAX, gain))

  // Detection phase states
  const [isListening, setIsListening] = useState(false)
  const [detectionStatus, setDetectionStatus] = useState<string | null>(null)
  const [detectionStatusType, setDetectionStatusType] = useState<'success' | 'error' | 'info' | 'warning'>('info')
  const [preambleDetected, setPreambleDetected] = useState(false)

  // Recording phase states
  const [isRecording, setIsRecording] = useState(false)
  const [recordingStatus, setRecordingStatus] = useState<string | null>(null)
  const [recordingStatusType, setRecordingStatusType] = useState<'success' | 'error' | 'info' | 'warning'>('info')
  const [recordingDuration, setRecordingDuration] = useState(0)
  const [recordingSamples, setRecordingSamples] = useState(0)

  // Post-recording states
  const [postambleDetected, setPostambleDetected] = useState(false)
  const [isDetecting, setIsDetecting] = useState(false)
  const [decodedText, setDecodedText] = useState<string | null>(null)
  const [isPlaying, setIsPlaying] = useState(false)

  // Audio I/O refs
  const processorRef = useRef<AudioWorkletNode | null>(null)
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const analyserRef = useRef<AnalyserNode | null>(null)
  const playbackSourceRef = useRef<AudioBufferSourceNode | null>(null)

  // Worker refs
  const preambleWorkerRef = useRef<Worker | null>(null)
  const postambleWorkerRef = useRef<Worker | null>(null)
  const decoderWorkerRef = useRef<Worker | null>(null)

  // Detection refs
  const resampleBufferRef = useRef<number[]>([])
  const allResampledSamplesRef = useRef<number[]>([]) // Keep all resampled samples for recording
  const samplesProcessedRef = useRef<number>(0)
  const volumeUpdateIntervalRef = useRef<number>(0)

  // Recording refs
  const recordedSamplesRef = useRef<number[]>([])
  const recordingStartTimeRef = useRef<number>(0)
  const recordingDurationIntervalRef = useRef<number>(0)
  const postambleSearchStartRef = useRef<number>(0)
  const recordingResampleBufferRef = useRef<number[]>([])
  const isRecordingRef = useRef<boolean>(false)
  const preambleDetectedRef = useRef<boolean>(false)
  const preamblePosInRecordingRef = useRef<number>(0)
  const autoGainAdjustmentRef = useRef<number>(1.0) // Persistent ref for gain adjustment

  // Cleanup workers on unmount
  useEffect(() => {
    return () => {
      if (preambleWorkerRef.current) {
        preambleWorkerRef.current.terminate()
        preambleWorkerRef.current = null
      }
      if (postambleWorkerRef.current) {
        postambleWorkerRef.current.terminate()
        postambleWorkerRef.current = null
      }
      if (decoderWorkerRef.current) {
        decoderWorkerRef.current.terminate()
        decoderWorkerRef.current = null
      }
    }
  }, [])

  // UI refs
  const [micVolume, setMicVolume] = useState(0)
  const [enableAutoGain, setEnableAutoGain] = useState(true)
  const [targetAmplitude, setTargetAmplitude] = useState(0.5)
  const [autoGainAdjustment, setAutoGainAdjustment] = useState(1.0)

  // Threshold settings
  const [preambleThreshold, setPreambleThreshold] = useState(0.4)
  const [postambleThreshold, setPostambleThreshold] = useState(0.4)

  const startListening = async () => {
    try {
      // Terminate old workers if they exist
      if (preambleWorkerRef.current) {
        preambleWorkerRef.current.terminate()
        preambleWorkerRef.current = null
      }
      if (postambleWorkerRef.current) {
        postambleWorkerRef.current.terminate()
        postambleWorkerRef.current = null
      }
      if (decoderWorkerRef.current) {
        decoderWorkerRef.current.terminate()
        decoderWorkerRef.current = null
      }

      // Initialize preamble detection worker
      const preambleWorker = new Worker(new URL('../workers/preambleDetectorWorker.ts', import.meta.url), {
        type: 'module'
      })
      preambleWorkerRef.current = preambleWorker

      let preambleWorkerReady = false
      preambleWorker.onmessage = (event) => {
        const { type } = event.data
        if (type === 'init_done' && !preambleWorkerReady) {
          preambleWorkerReady = true
          console.log('Preamble worker initialized')
        } else if (type === 'preamble_detected') {
          // Preamble detected in worker, trigger recording start
          if (!preambleDetectedRef.current && isRecordingRef.current === false) {
            console.log('Preamble detected from worker!')
            handlePreambleDetected()
          }
        } else if (type === 'error') {
          console.error('Preamble worker error:', event.data.error)
        }
      }

      // Initialize preamble detector with configurable threshold
      preambleWorker.postMessage({ type: 'init', threshold: preambleThreshold })

      // Initialize postamble detection worker
      const postambleWorker = new Worker(new URL('../workers/postambleDetectorWorker.ts', import.meta.url), {
        type: 'module'
      })
      postambleWorkerRef.current = postambleWorker

      let postambleWorkerReady = false
      postambleWorker.onmessage = (event) => {
        const { type } = event.data
        if (type === 'init_done' && !postambleWorkerReady) {
          postambleWorkerReady = true
          console.log('Postamble worker initialized')
        } else if (type === 'postamble_detected') {
          // Postamble detected in worker, stop recording
          if (isRecordingRef.current && !postambleDetected) {
            console.log('Postamble detected from worker!')
            isRecordingRef.current = false
            setPostambleDetected(true)
            stopRecording('Recording stopped (postamble detected)')
            // Trigger auto-decode after a brief delay
            setTimeout(() => {
              decodeRecordedAudio()
            }, 100)
          }
        } else if (type === 'error') {
          console.error('Postamble worker error:', event.data.error)
        }
      }

      // Initialize postamble detector with configurable threshold
      postambleWorker.postMessage({ type: 'init', threshold: postambleThreshold })

      // Initialize decoder worker
      const decoderWorker = new Worker(new URL('../workers/decoderWorker.ts', import.meta.url), {
        type: 'module'
      })
      decoderWorkerRef.current = decoderWorker

      decoderWorker.onmessage = (event) => {
        const { type } = event.data
        if (type === 'init_done') {
          console.log('Decoder worker initialized')
        } else if (type === 'decode_success') {
          const { text } = event.data
          setDecodedText(text)
          setRecordingStatus(`Decoded successfully: "${text}"`)
          setRecordingStatusType('success')
          setIsDetecting(false)
          console.log('Decode succeeded via worker!')
        } else if (type === 'decode_failed') {
          const { error } = event.data
          setRecordingStatus(`Decode failed: ${error}`)
          setRecordingStatusType('error')
          setIsDetecting(false)
          console.log('Decode failed:', error)
        } else if (type === 'error') {
          console.error('Decoder worker error:', event.data.error)
          setRecordingStatus(`Worker error: ${event.data.error}`)
          setRecordingStatusType('error')
          setIsDetecting(false)
        }
      }

      // Initialize decoder with thresholds
      decoderWorker.postMessage({ type: 'init', preambleThreshold, postambleThreshold })

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

      // Create analyser for volume visualization
      const analyser = audioContext.createAnalyser()
      analyser.fftSize = 2048
      analyserRef.current = analyser

      if (!audioContext.audioWorklet) {
        throw new Error('AudioWorklet API is not supported in this browser')
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

      // Reset all buffers and refs
      resampleBufferRef.current = []
      allResampledSamplesRef.current = []
      samplesProcessedRef.current = 0
      recordedSamplesRef.current = []
      recordingResampleBufferRef.current = []
      isRecordingRef.current = false
      preambleDetectedRef.current = false
      preamblePosInRecordingRef.current = 0
      postambleSearchStartRef.current = 0

      // Connect audio graph
      source.connect(analyser)
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

      // Reset all UI state
      setIsListening(true)
      setIsRecording(false)
      setDetectionStatus('Listening for preamble...')
      setDetectionStatusType('info')
      setPreambleDetected(false)
      setPostambleDetected(false)
      setDecodedText(null)
      setRecordingStatus(null)
      setRecordingDuration(0)
      setRecordingSamples(0)

      processor.port.onmessage = (event: MessageEvent<Float32Array>) => {
        const samples: number[] = Array.from(event.data)
        const actualSampleRate = audioContextRef.current?.sampleRate || 48000

        // Accumulate all raw samples for potential recording
        // This buffer holds samples during the listening phase
        if (!isRecordingRef.current) {
          resampleBufferRef.current.push(...samples)
        } else {
          // During recording, use the dedicated recording resample buffer
          recordingResampleBufferRef.current.push(...samples)
        }

        // Handle preamble detection phase
        if (!isRecordingRef.current && !preambleDetectedRef.current) {
          // Process accumulated samples in chunks for detection
          const chunkSize = 4096
          if (resampleBufferRef.current.length < chunkSize) {
            return // Wait for more samples
          }

          const chunk = resampleBufferRef.current.splice(0, chunkSize)
          let resampledChunk = chunk
          if (actualSampleRate !== TARGET_SAMPLE_RATE) {
            resampledChunk = resampleAudio(chunk, actualSampleRate, TARGET_SAMPLE_RATE)
          }

          // Keep track of ALL resampled samples for later recording
          allResampledSamplesRef.current.push(...resampledChunk)
          samplesProcessedRef.current += resampledChunk.length

          // Send to preamble worker for detection (async)
          if (preambleWorkerRef.current) {
            preambleWorkerRef.current.postMessage({
              type: 'add_samples',
              samples: new Float32Array(resampledChunk)
            })
          }

          // Periodically clear buffer to prevent unbounded growth
          if (samplesProcessedRef.current > MAX_BUFFER_SAMPLES) {
            if (preambleWorkerRef.current) {
              preambleWorkerRef.current.postMessage({ type: 'clear' })
            }
            samplesProcessedRef.current = 0
            resampleBufferRef.current = [] // Clear the raw sample buffer too
            allResampledSamplesRef.current = [] // Clear resampled samples too
          }
        } else if (isRecordingRef.current) {
          // Recording phase - process and save audio samples
          const chunkSize = 4096
          if (recordingResampleBufferRef.current.length < chunkSize) {
            return // Wait for more samples
          }

          const chunk = recordingResampleBufferRef.current.splice(0, chunkSize)
          let resampledChunk = chunk
          if (actualSampleRate !== TARGET_SAMPLE_RATE) {
            resampledChunk = resampleAudio(chunk, actualSampleRate, TARGET_SAMPLE_RATE)
          }

          // Dynamically refine gain so we keep hugging the target amplitude even if the
          // speaker volume shifts mid-frame (only if auto-gain is enabled)
          if (enableAutoGain) {
            const inputRms = calculateRMS(resampledChunk)
            if (inputRms > 0 && targetAmplitude > 0) {
              const estimatedOutput = inputRms * autoGainAdjustmentRef.current
              const errorRatio = Math.abs(estimatedOutput - targetAmplitude) / targetAmplitude
              if (errorRatio > AUTO_GAIN_TOLERANCE) {
                const desiredGain = clampGain(targetAmplitude / inputRms)
                const blendedGain =
                  autoGainAdjustmentRef.current +
                  (desiredGain - autoGainAdjustmentRef.current) * AUTO_GAIN_SMOOTHING
                const clampedGain = clampGain(blendedGain)
                if (Math.abs(clampedGain - autoGainAdjustmentRef.current) > 0.01) {
                  autoGainAdjustmentRef.current = clampedGain
                  setAutoGainAdjustment(clampedGain)
                } else {
                  autoGainAdjustmentRef.current = clampedGain
                }
              }
            }
          }

          const gainToApply = autoGainAdjustmentRef.current
          // Apply auto-gain adjustment and normalize samples with soft clipping
          const normalizedSamples = resampledChunk.map((sample) => applyAutoGain(sample, gainToApply))

          recordedSamplesRef.current.push(...normalizedSamples)
          setRecordingSamples(recordedSamplesRef.current.length)

          // Check if recording buffer exceeds safety limit
          if (recordedSamplesRef.current.length > MAX_RECORDING_SAMPLES) {
            isRecordingRef.current = false
            stopRecording('Recording stopped (buffer limit reached, attempting decode)')
            // Attempt to decode the recorded audio
            setTimeout(() => {
              decodeRecordedAudio()
            }, 100)
            return
          }

          // Try to detect postamble after we have enough samples
          // Skip the first 8000 samples (preamble + some data)
          if (postambleWorkerRef.current && recordedSamplesRef.current.length > 8000) {
            // Use windowing with overlap for better detection
            // Send a window of 4000 samples with 2000 sample overlap
            const windowSize = 4000
            const overlapSize = 2000
            let windowStart = postambleSearchStartRef.current

            // Initialize window start on first check
            if (windowStart === 0) {
              windowStart = 8000
            }

            // Only send if we have new samples beyond our previous window
            if (windowStart + windowSize <= recordedSamplesRef.current.length) {
              const windowEnd = windowStart + windowSize
              const windowSamples = recordedSamplesRef.current.slice(windowStart, windowEnd)

              postambleWorkerRef.current.postMessage({
                type: 'add_samples',
                samples: new Float32Array(windowSamples)
              })

              // Move window forward with overlap
              postambleSearchStartRef.current = windowEnd - overlapSize
            }
          }
        }
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to access microphone'
      setDetectionStatus(`Error: ${message}`)
      setDetectionStatusType('error')
    }
  }

  const handlePreambleDetected = () => {
    if (preambleDetectedRef.current || isRecordingRef.current) return

    preambleDetectedRef.current = true
    isRecordingRef.current = true
    setPreambleDetected(true)
    setDetectionStatus('Preamble detected! Recording...')
    setDetectionStatusType('success')
    setIsRecording(true)
    recordingStartTimeRef.current = Date.now()

    // Trim buffer to include just a small pre-roll before preamble
    const PREAMBLE_DURATION_MS = 250
    const PREAMBLE_SAMPLES = (TARGET_SAMPLE_RATE * PREAMBLE_DURATION_MS) / 1000
    const PRE_ROLL_MS = 100
    const PRE_ROLL_SAMPLES = (TARGET_SAMPLE_RATE * PRE_ROLL_MS) / 1000

    const preambleWindow = Math.min(PREAMBLE_SAMPLES, allResampledSamplesRef.current.length)
    const preambleStart = Math.max(0, allResampledSamplesRef.current.length - preambleWindow)
    const trimmedStart = Math.max(0, preambleStart - PRE_ROLL_SAMPLES)
    const allResampled = allResampledSamplesRef.current.slice(trimmedStart)
    const preambleSamples = allResampledSamplesRef.current.slice(preambleStart)
    allResampledSamplesRef.current = []
    resampleBufferRef.current = []

    // Calculate preamble amplitude (RMS of detected preamble only)
    const preambleAmplitude = calculateRMS(preambleSamples)

    // Calculate gain adjustment to reach target amplitude (only if auto-gain is enabled)
    let gainAdjustment = 1.0
    if (enableAutoGain && preambleAmplitude > 0) {
      gainAdjustment = targetAmplitude / preambleAmplitude
      gainAdjustment = clampGain(gainAdjustment)
    }
    autoGainAdjustmentRef.current = gainAdjustment
    setAutoGainAdjustment(gainAdjustment)

    // Normalize the accumulated resampled samples with gain adjustment
    const normalizedAccumulated = allResampled.map((sample) => applyAutoGain(sample, gainAdjustment))

    // Start recording with the preamble and everything before it
    recordedSamplesRef.current = normalizedAccumulated
    preamblePosInRecordingRef.current = normalizedAccumulated.length
    setRecordingDuration(0)
    setRecordingSamples(recordedSamplesRef.current.length)

    // Start recording duration timer
    recordingDurationIntervalRef.current = window.setInterval(() => {
      const elapsed = Math.floor((Date.now() - recordingStartTimeRef.current) / 1000)
      setRecordingDuration(elapsed)

      // Auto-stop at MAX_RECORDING_DURATION and attempt to decode
      if (elapsed >= MAX_RECORDING_DURATION) {
        isRecordingRef.current = false
        stopRecording(`Recording stopped (max ${MAX_RECORDING_DURATION}s reached, attempting decode)`)
        // Attempt to decode the recorded audio
        setTimeout(() => {
          decodeRecordedAudio()
        }, 100)
      }
    }, 100)

    // Clear preamble worker buffer for memory efficiency
    if (preambleWorkerRef.current) {
      preambleWorkerRef.current.postMessage({ type: 'clear' })
    }
    samplesProcessedRef.current = 0
  }

  const stopListening = () => {
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

    if (volumeUpdateIntervalRef.current) {
      clearInterval(volumeUpdateIntervalRef.current)
    }

    if (recordingDurationIntervalRef.current) {
      clearInterval(recordingDurationIntervalRef.current)
    }

    setIsListening(false)
    setIsRecording(false)
    setDetectionStatus('Stopped listening')
    setDetectionStatusType('info')
    autoGainAdjustmentRef.current = 1.0 // Reset auto-gain adjustment
    setAutoGainAdjustment(1.0)
  }

  const stopRecording = (message?: string) => {
    // Immediately disconnect audio to stop processing
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

    if (recordingDurationIntervalRef.current) {
      clearInterval(recordingDurationIntervalRef.current)
    }

    if (volumeUpdateIntervalRef.current) {
      clearInterval(volumeUpdateIntervalRef.current)
    }

    isRecordingRef.current = false
    setIsRecording(false)
    setIsListening(false)
    autoGainAdjustmentRef.current = 1.0 // Reset auto-gain adjustment
    setAutoGainAdjustment(1.0)
    if (message) {
      const type = postambleDetected ? 'success' : 'info'
      setRecordingStatus(message)
      setRecordingStatusType(type)
      setDetectionStatus(message)
      setDetectionStatusType(type)
    } else {
      setRecordingStatus(null)
      setDetectionStatus('Ready to listen')
      setDetectionStatusType('info')
    }
  }

  const saveWave = () => {
    if (recordedSamplesRef.current.length === 0) {
      setRecordingStatus('No audio recorded to save')
      setRecordingStatusType('error')
      return
    }

    try {
      const samplesToSave = recordedSamplesRef.current
      const wav = encodeWAV(samplesToSave, TARGET_SAMPLE_RATE)
      const blob = new Blob([wav], { type: 'audio/wav' })
      const url = URL.createObjectURL(blob)

      const a = document.createElement('a')
      a.href = url
      a.download = `preamble-postamble-${Date.now()}.wav`
      a.click()
      URL.revokeObjectURL(url)

      setRecordingStatus('Recording saved at 16kHz!')
      setRecordingStatusType('success')
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to save'
      setRecordingStatus(`Error: ${message}`)
      setRecordingStatusType('error')
    }
  }

  const decodeRecordedAudio = () => {
    if (recordedSamplesRef.current.length === 0) {
      setRecordingStatus('No audio recorded to decode')
      setRecordingStatusType('error')
      return
    }

    if (!decoderWorkerRef.current) {
      setRecordingStatus('Decoder worker not initialized')
      setRecordingStatusType('error')
      return
    }

    setIsDetecting(true)
    setRecordingStatus('Decoding...')
    setRecordingStatusType('info')

    // recordedSamplesRef.current is already normalized and resampled to 16kHz
    const resampledSamples = recordedSamplesRef.current

    // Send to decoder worker for async decoding
    decoderWorkerRef.current.postMessage({
      type: 'decode',
      samples: new Float32Array(resampledSamples)
    })
  }

  const encodeWAV = (samples: number[], sampleRate: number): ArrayBuffer => {
    const numChannels = 1
    const frame = samples.length
    const length = frame * numChannels * 2 + 44

    const arrayBuffer = new ArrayBuffer(length)
    const view = new DataView(arrayBuffer)

    const writeString = (offset: number, string: string) => {
      for (let i = 0; i < string.length; i++) {
        view.setUint8(offset + i, string.charCodeAt(i))
      }
    }

    writeString(0, 'RIFF')
    view.setUint32(4, 36 + frame * numChannels * 2, true)
    writeString(8, 'WAVE')
    writeString(12, 'fmt ')
    view.setUint32(16, 16, true)
    view.setUint16(20, 1, true)
    view.setUint16(22, numChannels, true)
    view.setUint32(24, sampleRate, true)
    view.setUint32(28, sampleRate * 2 * numChannels, true)
    view.setUint16(32, numChannels * 2, true)
    view.setUint16(34, 16, true)
    writeString(36, 'data')
    view.setUint32(40, frame * numChannels * 2, true)

    let offset = 44
    for (let i = 0; i < frame; i++) {
      let s = Math.max(-1, Math.min(1, samples[i]))
      view.setInt16(offset, s < 0 ? s * 0x8000 : s * 0x7fff, true)
      offset += 2
    }

    return arrayBuffer
  }

  const resetRecording = () => {
    // Reset all refs
    preambleDetectedRef.current = false
    isRecordingRef.current = false
    preamblePosInRecordingRef.current = 0
    recordedSamplesRef.current = []
    recordingResampleBufferRef.current = []
    autoGainAdjustmentRef.current = 1.0 // Reset auto-gain adjustment

    // Reset all state
    setPreambleDetected(false)
    setPostambleDetected(false)
    setDecodedText(null)
    setRecordingSamples(0)
    setRecordingDuration(0)
    setRecordingStatus(null)
    setDetectionStatus(null)
    setAutoGainAdjustment(1.0)
  }

  const playAudio = async () => {
    if (recordedSamplesRef.current.length === 0) {
      setRecordingStatus('No audio recorded to play')
      setRecordingStatusType('error')
      return
    }

    try {
      // Stop any currently playing audio
      if (playbackSourceRef.current) {
        playbackSourceRef.current.stop()
        playbackSourceRef.current = null
      }

      // Use existing audio context or create a new one
      let audioContext = audioContextRef.current
      if (!audioContext || audioContext.state === 'closed') {
        audioContext = new (window.AudioContext || (window as any).webkitAudioContext)()
        audioContextRef.current = audioContext
      }

      // Create AudioBuffer
      const audioBuffer = audioContext.createBuffer(1, recordedSamplesRef.current.length, TARGET_SAMPLE_RATE)
      const channelData = audioBuffer.getChannelData(0)
      channelData.set(recordedSamplesRef.current)

      // Create and play source
      const source = audioContext.createBufferSource()
      source.buffer = audioBuffer
      source.connect(audioContext.destination)
      playbackSourceRef.current = source

      setIsPlaying(true)
      setRecordingStatus('Playing audio...')
      setRecordingStatusType('info')

      // Handle when playback ends
      source.onended = () => {
        setIsPlaying(false)
        playbackSourceRef.current = null
        setRecordingStatus('Playback finished')
        setRecordingStatusType('success')
      }

      source.start(0)
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to play audio'
      setRecordingStatus(`Error: ${message}`)
      setRecordingStatusType('error')
    }
  }

  const stopAudio = () => {
    if (playbackSourceRef.current) {
      playbackSourceRef.current.stop()
      playbackSourceRef.current = null
      setIsPlaying(false)
      setRecordingStatus('Playback stopped')
      setRecordingStatusType('info')
    }
  }

  return (
    <div className="container">
      <div className="text-center mb-5">
        <h1>🎯 Preamble → Record → Postamble</h1>
        <p>Listen for preamble signal to auto-start recording, stop on postamble or after 30s, then save/decode</p>
      </div>

      <div className="card">
        <h2>Listening & Recording Settings</h2>

        <div className="mt-4">
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={enableAutoGain}
              onChange={(e) => setEnableAutoGain(e.target.checked)}
              disabled={isListening}
            />
            <strong>Enable Auto-Gain Adjustment</strong>
          </label>
          <small>Automatically adjust gain when preamble is detected to reach target amplitude</small>
        </div>

        {enableAutoGain && (
          <div className="mt-4">
            <label><strong>Target Preamble Amplitude</strong></label>
            <div className="flex items-center gap-3 mt-2">
              <input
                type="range"
                min="0.1"
                max="0.9"
                step="0.1"
                value={targetAmplitude}
                onChange={(e) => setTargetAmplitude(parseFloat(e.target.value))}
                disabled={isListening}
              />
              <span>{targetAmplitude.toFixed(1)}</span>
            </div>
            <small>Recommended: 0.5</small>
            {autoGainAdjustment !== 1.0 && !isListening && (
              <div style={{ marginTop: '0.5rem', color: '#059669' }}>
                Last adjustment: {autoGainAdjustment.toFixed(2)}x
              </div>
            )}
          </div>
        )}

        <div className="mt-4">
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

        <div className="mt-4">
          <label><strong>Postamble Detection Threshold</strong></label>
          <div className="flex items-center gap-3 mt-2">
            <input
              type="range"
              min="0.1"
              max="0.9"
              step="0.05"
              value={postambleThreshold}
              onChange={(e) => setPostambleThreshold(parseFloat(e.target.value))}
              disabled={isListening}
            />
            <span>{postambleThreshold.toFixed(2)}</span>
          </div>
          <small>Lower values = more sensitive. Default: 0.4</small>
        </div>

        <div className="mt-4">
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

        <div className="mt-4 flex gap-3">
          <button
            onClick={startListening}
            disabled={isListening}
            className="btn-primary w-full"
          >
            Start Listening
          </button>
          {isListening && (
            <button
              onClick={stopListening}
              className="btn-secondary w-full"
            >
              Stop
            </button>
          )}
        </div>

        {detectionStatus && <Status message={detectionStatus} type={detectionStatusType} />}
      </div>

      {(preambleDetected || recordingSamples > 0) && (
        <div className="card">
          <h2>Recording Status</h2>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '1rem', marginBottom: '1rem' }}>
            <div style={{ background: '#f7fafc', padding: '1rem', borderRadius: '0.5rem' }}>
              <div style={{ color: '#999', fontSize: '0.9rem', marginBottom: '0.5rem' }}>PREAMBLE</div>
              <div style={{ fontSize: '1.2rem', fontWeight: 'bold', color: preambleDetected ? '#48bb78' : '#999' }}>
                {preambleDetected ? '✓ Detected' : '○ Waiting'}
              </div>
            </div>
            <div style={{ background: '#f7fafc', padding: '1rem', borderRadius: '0.5rem' }}>
              <div style={{ color: '#999', fontSize: '0.9rem', marginBottom: '0.5rem' }}>POSTAMBLE</div>
              <div style={{ fontSize: '1.2rem', fontWeight: 'bold', color: postambleDetected ? '#48bb78' : '#999' }}>
                {postambleDetected ? '✓ Detected' : isRecording ? '○ Listening' : '○ Not detected'}
              </div>
            </div>
          </div>

          <div style={{ background: '#f7fafc', padding: '1rem', borderRadius: '0.5rem', marginBottom: '1rem' }}>
            <div>Duration: {recordingDuration}s / {MAX_RECORDING_DURATION}s</div>
            <div>Samples: {recordingSamples}</div>
            <div>Applied Gain: {autoGainAdjustment.toFixed(2)}x</div>
          </div>

          {recordingStatus && <Status message={recordingStatus} type={recordingStatusType} />}
        </div>
      )}

      {!isRecording && recordingSamples > 0 && (
        <div className="card">
          <h2>Post-Recording Actions</h2>

          <div className="mt-4 flex gap-3">
            {!isPlaying ? (
              <button
                onClick={playAudio}
                className="btn-secondary w-full"
              >
                ▶️ Play Audio
              </button>
            ) : (
              <button
                onClick={stopAudio}
                className="btn-secondary w-full"
                style={{ background: '#dc2626' }}
              >
                ⏹️ Stop Playback
              </button>
            )}
            <button
              onClick={saveWave}
              className="btn-secondary w-full"
            >
              💾 Download WAV
            </button>
            <button
              onClick={decodeRecordedAudio}
              disabled={isDetecting}
              className="btn-primary w-full"
            >
              {isDetecting ? 'Decoding...' : '🔍 Decode'}
            </button>
          </div>

          <button
            onClick={resetRecording}
            className="btn-secondary w-full mt-3"
          >
            🔄 Reset & Listen Again
          </button>
        </div>
      )}

      {(decodedText !== null || (detectionStatus && isDetecting === false && recordingSamples > 0)) && (
        <div className="card">
          <h2>Detection & Decode Result</h2>

          {decodedText !== null && (
            <div style={{
              background: '#f7fafc',
              padding: '1rem',
              borderRadius: '0.5rem',
              wordBreak: 'break-word',
              fontFamily: 'monospace',
              minHeight: '80px',
              marginBottom: '1rem'
            }}>
              <strong>Decoded Message:</strong><br/>
              {decodedText}
            </div>
          )}

          {detectionStatus && <Status message={detectionStatus} type={detectionStatusType} />}
        </div>
      )}

      <button onClick={() => navigate('/')} className="btn-secondary">
        ← Back to Home
      </button>
    </div>
  )
}

export default PreamblePostambleRecordPage
