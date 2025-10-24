import React, { useState, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { PostambleDetector, PreambleDetector } from '../utils/wasm'
import { resampleAudio } from '../utils/audio'
import Status from '../components/Status'

const TARGET_SAMPLE_RATE = 16000
type DetectionMode = 'preamble' | 'postamble'

const AmplePage: React.FC = () => {
  const navigate = useNavigate()
  const [mode, setMode] = useState<DetectionMode>('preamble')
  const [threshold, setThreshold] = useState(0.4)
  const [isListening, setIsListening] = useState(false)
  const [status, setStatus] = useState<string | null>(null)
  const [statusType, setStatusType] = useState<'success' | 'error' | 'info' | 'warning'>('info')
  const [bufferSize, setBufferSize] = useState(0)
  const [requiredSize, setRequiredSize] = useState(0)
  const [detections, setDetections] = useState<string[]>([])

  const processorRef = useRef<ScriptProcessorNode | null>(null)
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const detectorRef = useRef<PostambleDetector | PreambleDetector | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const resampleBufferRef = useRef<number[]>([])
  const samplesProcessedRef = useRef<number>(0)

  const startListening = async () => {
    try {
      // Create appropriate detector based on mode
      const detector = mode === 'preamble'
        ? new PreambleDetector(threshold)
        : new PostambleDetector(threshold)
      detectorRef.current = detector

      const stream = await navigator.mediaDevices.getUserMedia({ audio: true })

      const audioContext = new (window.AudioContext || (window as any).webkitAudioContext)()
      audioContextRef.current = audioContext
      const source = audioContext.createMediaStreamSource(stream)
      const processor = audioContext.createScriptProcessor(4096, 1, 1)

      sourceRef.current = source
      processorRef.current = processor
      streamRef.current = stream
      resampleBufferRef.current = []
      samplesProcessedRef.current = 0

      source.connect(processor)
      processor.connect(audioContext.destination)

      setIsListening(true)
      const modeLabel = mode === 'preamble' ? 'preamble' : 'postamble'
      setStatus(`Listening for ${modeLabel}...`)
      setStatusType('info')
      const requiredSizeValue = mode === 'preamble'
        ? PreambleDetector.required_size()
        : PostambleDetector.required_size()
      setRequiredSize(requiredSizeValue)
      setDetections([])

      processor.onaudioprocess = (event: AudioProcessingEvent) => {
        const samples = Array.from((event as any).inputBuffer.getChannelData(0))

        // Resample audio to 16kHz for consistent detection
        const actualSampleRate = audioContextRef.current?.sampleRate || 48000
        let resampledSamples = samples
        if (actualSampleRate !== TARGET_SAMPLE_RATE) {
          // Accumulate samples for batch resampling to reduce artifacts
          resampleBufferRef.current.push(...samples)

          // Process in chunks of 4096 samples at original rate (reduces resampling artifacts)
          const chunkSize = 4096
          if (resampleBufferRef.current.length < chunkSize) {
            return // Wait for more samples to accumulate
          }

          // Take a chunk and resample
          const chunk = resampleBufferRef.current.splice(0, chunkSize)
          resampledSamples = resampleAudio(chunk, actualSampleRate, TARGET_SAMPLE_RATE)
        }

        const position = detector.add_samples(new Float32Array(resampledSamples))
        samplesProcessedRef.current += resampledSamples.length

        // Periodically clear buffer to prevent unbounded growth (every ~5 seconds at 16kHz)
        // Clear if we've accumulated more than 80k samples without detection
        const MAX_BUFFER_SAMPLES = 80000
        if (samplesProcessedRef.current > MAX_BUFFER_SAMPLES) {
          detector.clear()
          samplesProcessedRef.current = 0
        }

        // Handle detection
        if (position >= 0) {
          const timestamp = new Date().toLocaleTimeString()
          const modeLabel = mode === 'preamble' ? 'Preamble' : 'Postamble'
          const detection = `${timestamp}: ${modeLabel} detected at position ${position}`
          setDetections((prev) => [detection, ...prev.slice(0, 9)])
          setStatus(`${modeLabel} detected!`)
          setStatusType('success')

          // Clear detector buffer to prevent unbounded growth
          detector.clear()
          samplesProcessedRef.current = 0
        }

        // Update buffer info AFTER clearing
        const size = detector.buffer_size()
        setBufferSize(size)

        if (position >= 0) {
          setTimeout(() => {
            const modeLabel = mode === 'preamble' ? 'preamble' : 'postamble'
            setStatus(`Listening for ${modeLabel}...`)
            setStatusType('info')
          }, 2000)
        }
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to access microphone'
      setStatus(`Error: ${message}`)
      setStatusType('error')
    }
  }

  const stopListening = () => {
    if (processorRef.current && sourceRef.current && streamRef.current) {
      processorRef.current.disconnect()
      sourceRef.current.disconnect()
      streamRef.current.getTracks().forEach((track) => track.stop())
    }

    setIsListening(false)
    setStatus('Stopped listening')
    setStatusType('info')
  }

  const bufferProgress = requiredSize > 0 ? (bufferSize / requiredSize) * 100 : 0

  return (
    <div className="container">
      <div className="text-center mb-5">
        <h1>🎯 Signal Detection (Ample)</h1>
        <p id="modeDescription">
          {mode === 'preamble'
            ? 'Real-time detection of ascending chirp preamble (200Hz → 4000Hz)'
            : 'Real-time detection of descending chirp postamble (4000Hz → 200Hz)'}
        </p>
      </div>

      <div className="card">
        <h2>Detection Mode</h2>

        <div className="mt-4">
          <label><strong>Select Detection Mode</strong></label>
          <div style={{ display: 'flex', gap: '1rem', marginTop: '0.5rem' }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', cursor: 'pointer' }}>
              <input
                type="radio"
                name="detectionMode"
                value="preamble"
                checked={mode === 'preamble'}
                onChange={(e) => {
                  setMode(e.target.value as DetectionMode)
                  setDetections([])
                }}
                disabled={isListening}
              />
              Preamble (200Hz → 4000Hz)
            </label>
            <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', cursor: 'pointer' }}>
              <input
                type="radio"
                name="detectionMode"
                value="postamble"
                checked={mode === 'postamble'}
                onChange={(e) => {
                  setMode(e.target.value as DetectionMode)
                  setDetections([])
                }}
                disabled={isListening}
              />
              Postamble (4000Hz → 200Hz)
            </label>
          </div>
        </div>

        <h2 style={{ marginTop: '2rem' }}>Detection Settings</h2>

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
              className="btn-secondary w-full mt-3"
            >
              Stop Listening
            </button>
          )}
        </div>

        {status && <Status message={status} type={statusType} />}

        {isListening && detections.length > 0 && (
              <div className="mt-4">
                <p><strong>Detection History:</strong></p>
                <div
                  style={{
                    background: '#f7fafc',
                    padding: '1rem',
                    borderRadius: '0.5rem',
                    maxHeight: '200px',
                    overflowY: 'auto',
                    fontFamily: 'monospace',
                    fontSize: '0.9rem',
                  }}
                >
                  {detections.map((detection, idx) => (
                    <div key={idx}>{detection}</div>
                  ))}
                </div>
              </div>
            )}
      </div>

      <button onClick={() => navigate('/')} className="btn-secondary">
        ← Back to Home
      </button>
    </div>
  )
}

export default AmplePage
