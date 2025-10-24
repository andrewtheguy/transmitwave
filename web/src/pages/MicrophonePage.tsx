import React, { useState, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { MicrophoneListener } from '../utils/wasm'
import Status from '../components/Status'

const MicrophonePage: React.FC = () => {
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

  const startListening = async () => {
    try {
      const listener = new MicrophoneListener(threshold)
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
      setStatus('Listening for preamble...')
      setStatusType('info')
      setRequiredSize(listener.required_size())
      setDetections([])

      processor.onaudioprocess = (event: AudioProcessingEvent) => {
        const samples = event.inputData.getChannelData(0)
        const position = listener.add_samples(samples)

        // Update buffer info
        const size = listener.buffer_size()
        setBufferSize(size)

        // Handle detection
        if (position >= 0) {
          const timestamp = new Date().toLocaleTimeString()
          const detection = `${timestamp}: Detected at position ${position}`
          setDetections((prev) => [detection, ...prev.slice(0, 9)])
          setStatus('Preamble detected!')
          setStatusType('success')

          setTimeout(() => {
            setStatus('Listening for preamble...')
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
        <h1>🎤 Preamble Detection</h1>
        <p>Real-time detection of ascending chirp preamble (200Hz → 4000Hz)</p>
      </div>

      <div className="card">
        <h2>Microphone Settings</h2>

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
            />
            <span>{threshold.toFixed(1)}</span>
          </div>
          <small>Higher values require stronger preamble detection. Recommended: 0.4</small>
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

export default MicrophonePage
