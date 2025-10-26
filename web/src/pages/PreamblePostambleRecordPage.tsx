import React, { useState, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { PreambleDetector, PostambleDetector, createDecoder } from '../utils/wasm'
import { resampleAudio } from '../utils/audio'
import Status from '../components/Status'

const MAX_RECORDING_DURATION = 30
const TARGET_SAMPLE_RATE = 16000
const MAX_BUFFER_SAMPLES = 80000 // Listening phase buffer cap (~5 seconds at 16kHz)
const MAX_RECORDING_SAMPLES = 480000 // Recording phase buffer cap (~30 seconds at 16kHz)
const PREAMBLE_DURATION_MS = 250
const PREAMBLE_SAMPLES = (TARGET_SAMPLE_RATE * PREAMBLE_DURATION_MS) / 1000 // 4000
const PRE_ROLL_MS = 100
const PRE_ROLL_SAMPLES = (TARGET_SAMPLE_RATE * PRE_ROLL_MS) / 1000 // keep ~0.1s before preamble for safety
const MIC_PROCESSOR_URL = new URL('../worklets/mic-processor.ts', import.meta.url)
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
  const [threshold, setThreshold] = useState(0.4)
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

  // Audio I/O refs
  const processorRef = useRef<AudioWorkletNode | null>(null)
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const analyserRef = useRef<AnalyserNode | null>(null)

  // Detection refs
  const detectorRef = useRef<PreambleDetector | null>(null)
  const resampleBufferRef = useRef<number[]>([])
  const allResampledSamplesRef = useRef<number[]>([]) // Keep all resampled samples for recording
  const samplesProcessedRef = useRef<number>(0)
  const volumeUpdateIntervalRef = useRef<number>(0)

  // Recording refs
  const recordedSamplesRef = useRef<number[]>([])
  const recordingStartTimeRef = useRef<number>(0)
  const recordingDurationIntervalRef = useRef<number>(0)
  const postambleDetectorRef = useRef<PostambleDetector | null>(null)
  const postambleSearchStartRef = useRef<number>(0)
  const recordingResampleBufferRef = useRef<number[]>([])
  const isRecordingRef = useRef<boolean>(false)
  const preambleDetectedRef = useRef<boolean>(false)
  const preamblePosInRecordingRef = useRef<number>(0)
  const autoGainAdjustmentRef = useRef<number>(1.0) // Persistent ref for gain adjustment

  // UI refs
  const [micVolume, setMicVolume] = useState(0)
  const [targetAmplitude, setTargetAmplitude] = useState(0.5)
  const [autoGainAdjustment, setAutoGainAdjustment] = useState(1.0)

  const startListening = async () => {
    try {
      // Create preamble detector
      const detector = new PreambleDetector(threshold)
      detectorRef.current = detector

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

      await audioContext.audioWorklet.addModule(MIC_PROCESSOR_URL)
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
      postambleDetectorRef.current = null
      detectorRef.current = new PreambleDetector(threshold)

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

          const position = detector.add_samples(new Float32Array(resampledChunk))
          samplesProcessedRef.current += resampledChunk.length

          // Periodically clear buffer to prevent unbounded growth
          if (samplesProcessedRef.current > MAX_BUFFER_SAMPLES) {
            detector.clear()
            samplesProcessedRef.current = 0
            resampleBufferRef.current = [] // Clear the raw sample buffer too
            allResampledSamplesRef.current = [] // Clear resampled samples too
          }

          // Preamble detected - start recording
          if (position >= 0) {
            preambleDetectedRef.current = true
            isRecordingRef.current = true
            setPreambleDetected(true)
            setDetectionStatus('Preamble detected! Recording...')
            setDetectionStatusType('success')
            setIsRecording(true)
            recordingStartTimeRef.current = Date.now()

            // Trim buffer to include just a small pre-roll before preamble so we don't
            // dilute the RMS calculation with long stretches of silence.
            const preambleWindow = Math.min(PREAMBLE_SAMPLES, allResampledSamplesRef.current.length)
            const preambleStart = Math.max(0, allResampledSamplesRef.current.length - preambleWindow)
            const trimmedStart = Math.max(0, preambleStart - PRE_ROLL_SAMPLES)
            const allResampled = allResampledSamplesRef.current.slice(trimmedStart)
            const preambleSamples = allResampledSamplesRef.current.slice(preambleStart)
            allResampledSamplesRef.current = [] // Clear for next phase
            resampleBufferRef.current = [] // Clear the raw buffer too

            // Calculate preamble amplitude (RMS of detected preamble only)
            const preambleAmplitude = calculateRMS(preambleSamples)

            // Calculate gain adjustment to reach target amplitude
            // If preamble is 0.2 and target is 0.5, we need to apply 0.5/0.2 = 2.5x gain
            let gainAdjustment = 1.0
            if (preambleAmplitude > 0) {
              gainAdjustment = targetAmplitude / preambleAmplitude
              // Clamp gain adjustment to reasonable range
              gainAdjustment = clampGain(gainAdjustment)
            }
            // Store in both ref (for consistent use in callbacks) and state (for UI display)
            autoGainAdjustmentRef.current = gainAdjustment
            setAutoGainAdjustment(gainAdjustment)

            // Normalize the accumulated resampled samples with gain adjustment
            const normalizedAccumulated = allResampled.map((sample) => applyAutoGain(sample, gainAdjustment))

            // Start recording with the preamble and everything before it
            recordedSamplesRef.current = normalizedAccumulated
            preamblePosInRecordingRef.current = normalizedAccumulated.length
            setRecordingDuration(0)
            setRecordingSamples(recordedSamplesRef.current.length)

            // Initialize postamble detector for later
            postambleDetectorRef.current = new PostambleDetector(threshold)
            postambleSearchStartRef.current = 0

            // Start recording duration timer
            recordingDurationIntervalRef.current = window.setInterval(() => {
              const elapsed = Math.floor((Date.now() - recordingStartTimeRef.current) / 1000)
              setRecordingDuration(elapsed)

              // Auto-stop at MAX_RECORDING_DURATION
              if (elapsed >= MAX_RECORDING_DURATION) {
                stopRecording(`Recording stopped (max ${MAX_RECORDING_DURATION}s reached)`)
              }
            }, 100)

            // Clear detector for memory efficiency
            detector.clear()
            samplesProcessedRef.current = 0
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
          // speaker volume shifts mid-frame.
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

          const gainToApply = autoGainAdjustmentRef.current
          // Apply auto-gain adjustment and normalize samples with soft clipping
          const normalizedSamples = resampledChunk.map((sample) => applyAutoGain(sample, gainToApply))

          recordedSamplesRef.current.push(...normalizedSamples)
          setRecordingSamples(recordedSamplesRef.current.length)

          // Check if recording buffer exceeds safety limit
          if (recordedSamplesRef.current.length > MAX_RECORDING_SAMPLES) {
            isRecordingRef.current = false
            stopRecording('Recording stopped (buffer limit reached)')
            return
          }

          // Try to detect postamble after we have enough samples
          // Skip the first 8000 samples (preamble + some data)
          if (postambleDetectorRef.current && recordedSamplesRef.current.length > 8000) {
            // Initialize search start if not already done
            if (postambleSearchStartRef.current === 0 && recordedSamplesRef.current.length >= 8000) {
              postambleSearchStartRef.current = 8000
            }

            // Feed only new samples since last check to detector
            if (postambleSearchStartRef.current < recordedSamplesRef.current.length) {
              const newSamples = recordedSamplesRef.current.slice(postambleSearchStartRef.current)
              const postamblePos = postambleDetectorRef.current.add_samples(new Float32Array(newSamples))
              postambleSearchStartRef.current = recordedSamplesRef.current.length

              if (postamblePos >= 0) {
                // Postamble detected - stop recording and auto-decode
                isRecordingRef.current = false
                setPostambleDetected(true)
                stopRecording('Recording stopped (postamble detected)')

                // Trigger auto-decode after a brief delay to ensure state is updated
                setTimeout(() => {
                  decodeRecordedAudio()
                }, 100)
              }
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

  const decodeRecordedAudio = async () => {
    if (recordedSamplesRef.current.length === 0) {
      setRecordingStatus('No audio recorded to decode')
      setRecordingStatusType('error')
      return
    }

    try {
      setIsDetecting(true)
      setRecordingStatus('Decoding...')
      setRecordingStatusType('info')

      // recordedSamplesRef.current is already normalized and resampled to 16kHz
      const resampledSamples = recordedSamplesRef.current

      // Decode
      const decoder = await createDecoder()
      const data = decoder.decode(new Float32Array(resampledSamples))
      const text = new TextDecoder().decode(data)

      setDecodedText(text)
      setRecordingStatus(`Decoded successfully: "${text}"`)
      setRecordingStatusType('success')
    } catch (error) {
      let message = 'Decode failed'
      if (error instanceof Error) {
        message = error.message
      } else if (typeof error === 'string') {
        message = error
      }
      console.error('Decode error:', error)
      setRecordingStatus(message)
      setRecordingStatusType('error')
      setDetectionStatus(`Decode error: ${message}`)
      setDetectionStatusType('error')
    } finally {
      setIsDetecting(false)
    }
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
    postambleDetectorRef.current = null
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

  return (
    <div className="container">
      <div className="text-center mb-5">
        <h1>🎯 Preamble → Record → Postamble</h1>
        <p>Listen for preamble signal to auto-start recording, stop on postamble or after 30s, then save/decode</p>
      </div>

      <div className="card" style={{ background: '#fef3c7', borderLeft: '4px solid #f59e0b', marginBottom: '2rem' }}>
        <h3 style={{ color: '#92400e', marginTop: 0 }}>⚠️ Technical Debt</h3>
        <p style={{ color: '#92400e', marginBottom: 0 }}>
          <strong>Threshold Inconsistency:</strong> The web UI allows setting a custom detection threshold (0.1-0.9),
          but the backend decoder re-detects preamble/postamble signals with its own hardcoded threshold instead of
          skipping detection and decoding directly. This can cause inconsistencies if the UI threshold differs from the
          backend's detection threshold. Future improvement: Backend should accept pre-detected boundaries or disable
          re-detection when decoding already-captured audio clips.
        </p>
      </div>

      <div className="card">
        <h2>Listening & Recording Settings</h2>

        <div className="mt-4">
          <label><strong>Detection Threshold</strong></label>
          <div className="flex items-center gap-3 mt-2">
            <input
              type="range"
              min="0.1"
              max="0.9"
              step="0.1"
              value={threshold}
              onChange={(e) => setThreshold(parseFloat(e.target.value))}
              disabled={isListening}
            />
            <span>{threshold.toFixed(1)}</span>
          </div>
          <small>Higher values require stronger detection. Recommended: 0.4</small>
        </div>

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
          <small>Auto-adjust gain when preamble is detected to reach this amplitude. Recommended: 0.5</small>
          {autoGainAdjustment !== 1.0 && !isListening && (
            <div style={{ marginTop: '0.5rem', color: '#059669' }}>
              Last adjustment: {autoGainAdjustment.toFixed(2)}x
            </div>
          )}
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
