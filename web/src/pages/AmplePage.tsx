import React, { useState, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { MicrophoneListener, PostambleDetector } from '../utils/wasm'
import Status from '../components/Status'

const PostamblePage: React.FC = () => {
  const navigate = useNavigate()
  const [detectionType, setDetectionType] = useState<'preamble' | 'postamble'>('preamble')
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
  const detectorRef = useRef<MicrophoneListener | PostambleDetector | null>(null)

  const startListening = async () => {
    try {
      const detector = detectionType === 'preamble'
        ? new MicrophoneListener(threshold)
        : new PostambleDetector(threshold)
      detectorRef.current = detector

      const stream = await navigator.mediaDevices.getUserMedia({ audio: true })

      const audioContext = new (window.AudioContext || (window as any).webkitAudioContext)()
      const source = audioContext.createMediaStreamSource(stream)
      const processor = audioContext.createScriptProcessor(4096, 1, 1)

      sourceRef.current = source
      processorRef.current = processor
      streamRef.current = stream

      source.connect(processor)
      processor.connect(audioContext.destination)

      setIsListening(true)
      const typeLabel = detectionType === 'preamble' ? 'preamble' : 'postamble'
      setStatus(`Listening for ${typeLabel}...`)
      setStatusType('info')
      setRequiredSize(detector.required_size())
      setDetections([])

      processor.onaudioprocess = (event: AudioProcessingEvent) => {
        const samples = event.inputData.getChannelData(0)
        const position = detector.add_samples(samples)

        // Update buffer info
        const size = detector.buffer_size()
        setBufferSize(size)

        // Handle detection
        if (position >= 0) {
          const timestamp = new Date().toLocaleTimeString()
          const typeLabel = detectionType === 'preamble' ? 'Preamble' : 'Postamble'
          const detection = `${timestamp}: ${typeLabel} detected at position ${position}`
          setDetections((prev) => [detection, ...prev.slice(0, 9)])
          setStatus(`${typeLabel} detected!`)
          setStatusType('success')

          setTimeout(() => {
            setStatus(`Listening for ${detectionType}...`)
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

  const getDescription = () => {
    if (detectionType === 'preamble') {
      return 'Real-time detection of ascending chirp preamble (200Hz → 4000Hz)'
    } else {
      return 'Real-time detection of descending chirp postamble (4000Hz → 200Hz)'
    }
  }

  return (
    <div className="container">
      <div className="text-center mb-5">
        <h1>🎯 Preamble & Postamble Detection</h1>
        <p>{getDescription()}</p>
      </div>

      <div className="card">
        <h2>Detection Settings</h2>

        <div className="mt-4">
          <label><strong>Detection Type</strong></label>
          <div className="flex gap-3 mt-2">
            <button
              onClick={() => setDetectionType('preamble')}
              className={detectionType === 'preamble' ? 'btn-primary' : 'btn-secondary'}
              style={{ flex: 1 }}
              disabled={isListening}
            >
              📈 Preamble
            </button>
            <button
              onClick={() => setDetectionType('postamble')}
              className={detectionType === 'postamble' ? 'btn-primary' : 'btn-secondary'}
              style={{ flex: 1 }}
              disabled={isListening}
            >
              📉 Postamble
            </button>
          </div>
        </div>

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

        {isListening && (
          <>
            <div className="mt-4">
              <p><strong>Buffer Status:</strong></p>
              <div style={{ background: '#f7fafc', padding: '1rem', borderRadius: '0.5rem' }}>
                <div>Buffer: {bufferSize} / {requiredSize} samples</div>
                <div style={{ background: 'var(--border-color)', height: '8px', borderRadius: '4px', marginTop: '0.5rem' }}>
                  <div
                    style={{
                      background: 'var(--primary-color)',
                      height: '100%',
                      borderRadius: '4px',
                      width: `${bufferProgress}%`,
                      transition: 'width 0.2s',
                    }}
                  />
                </div>
              </div>
            </div>

            {detections.length > 0 && (
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
          </>
        )}
      </div>

      <button onClick={() => navigate('/')} className="btn-secondary">
        ← Back to Home
      </button>
    </div>
  )
}

export default PostamblePage
