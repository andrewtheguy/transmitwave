import React, { useState, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { PostambleDetector } from '../utils/wasm'
import { resampleAudio } from '../utils/audio'
import Status from '../components/Status'

const TARGET_SAMPLE_RATE = 16000

const PostamblePage: React.FC = () => {
  const navigate = useNavigate()
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
  const detectorRef = useRef<PostambleDetector | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const resampleBufferRef = useRef<number[]>([])
  const samplesProcessedRef = useRef<number>(0)

  const startListening = async () => {
    try {
      const detector = new PostambleDetector(threshold)
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
      setStatus('Listening for postamble...')
      setStatusType('info')
      setRequiredSize(PostambleDetector.required_size())
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
          const detection = `${timestamp}: Postamble detected at position ${position}`
          setDetections((prev) => [detection, ...prev.slice(0, 9)])
          setStatus('Postamble detected!')
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
            setStatus('Listening for postamble...')
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
        <h1>📉 Postamble Detection</h1>
        <p>Real-time detection of descending chirp postamble (4000Hz → 200Hz)</p>
      </div>

      <div className="card">
        <h2>Detection Settings</h2>

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

export default PostamblePage
