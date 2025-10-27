import React, { useState, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { PostambleDetector, PreambleDetector } from '../utils/wasm'
import { resampleAudio } from '../utils/audio'
import Status from '../components/Status'
import { getMicProcessorUrl } from '../utils/mic-processor-inline'

const TARGET_SAMPLE_RATE = 16000
type DetectionMode = 'preamble' | 'postamble'

const AmplePage: React.FC = () => {
  const navigate = useNavigate()
  const [mode, setMode] = useState<DetectionMode>('preamble')
  const [threshold, setThreshold] = useState<number | null>(null) // null = adaptive
  const [isListening, setIsListening] = useState(false)
  const [status, setStatus] = useState<string | null>(null)
  const [statusType, setStatusType] = useState<'success' | 'error' | 'info' | 'warning'>('info')
  const [bufferSize, setBufferSize] = useState(0)
  const [requiredSize, setRequiredSize] = useState(0)
  const [detections, setDetections] = useState<string[]>([])

  const processorRef = useRef<AudioWorkletNode | null>(null)
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const detectorRef = useRef<PostambleDetector | PreambleDetector | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const resampleBufferRef = useRef<number[]>([])
  const samplesProcessedRef = useRef<number>(0)
  const gainNodeRef = useRef<GainNode | null>(null)
  const analyserRef = useRef<AnalyserNode | null>(null)
  const volumeUpdateIntervalRef = useRef<number>(0)
  const [micVolume, setMicVolume] = useState(0)
  const [volumeGain, setVolumeGain] = useState(1)

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

      // Create gain node for volume control
      const gainNode = audioContext.createGain()
      gainNode.gain.value = volumeGain
      gainNodeRef.current = gainNode

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
      resampleBufferRef.current = []
      samplesProcessedRef.current = 0

      // Connect with gain and analyser
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
      const modeLabel = mode === 'preamble' ? 'preamble' : 'postamble'
      setStatus(`Listening for ${modeLabel}...`)
      setStatusType('info')
      const requiredSizeValue = mode === 'preamble'
        ? PreambleDetector.required_size()
        : PostambleDetector.required_size()
      setRequiredSize(requiredSizeValue)
      setDetections([])

      processor.port.onmessage = (event: MessageEvent<Float32Array>) => {
        const samples = Array.from(event.data)

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
    if (processorRef.current) {
      processorRef.current.port.onmessage = null
      processorRef.current.disconnect()
      processorRef.current = null
    }

    if (analyserRef.current) {
      analyserRef.current.disconnect()
    }

    if (gainNodeRef.current) {
      gainNodeRef.current.disconnect()
      gainNodeRef.current = null
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

    resampleBufferRef.current = []
    samplesProcessedRef.current = 0

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
          <label><strong>Microphone Volume</strong></label>
          <div className="flex items-center gap-3 mt-2">
            <input
              type="range"
              min="0.5"
              max="3"
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
          <small>Amplify microphone input (0.5x to 3x). Recommended: 1.0x</small>
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

        <div className="mt-4">
          <label><strong>Detection Threshold</strong></label>
          <div className="flex items-center gap-3 mt-2">
            <select
              value={threshold === null ? 'adaptive' : threshold.toString()}
              onChange={(e) => setThreshold(e.target.value === 'adaptive' ? null : parseFloat(e.target.value))}
              disabled={isListening}
              style={{ flex: 1 }}
            >
              <option value="adaptive">Adaptive (auto-adjust based on signal)</option>
              {[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9].map(v => (
                <option key={v} value={v}>{v.toFixed(1)} (fixed)</option>
              ))}
            </select>
          </div>
          <small>Adaptive: automatically adjusts based on signal strength | Fixed: use specific threshold value</small>
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
