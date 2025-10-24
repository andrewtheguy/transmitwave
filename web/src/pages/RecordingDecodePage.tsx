import React, { useState, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { MicrophoneListener, PostambleDetector, createDecoder } from '../utils/wasm'
import { resampleAudio } from '../utils/audio'
import Status from '../components/Status'

const RecordingDecodePage: React.FC = () => {
  const navigate = useNavigate()
  const [threshold, setThreshold] = useState(0.4)
  const [isRecording, setIsRecording] = useState(false)
  const [status, setStatus] = useState<string | null>(null)
  const [statusType, setStatusType] = useState<'success' | 'error' | 'info' | 'warning'>('info')
  const [duration, setDuration] = useState(0)
  const [samples, setSamples] = useState(0)
  const [decodedText, setDecodedText] = useState<string | null>(null)

  const processorRef = useRef<ScriptProcessorNode | null>(null)
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const recordedSamplesRef = useRef<number[]>([])
  const startTimeRef = useRef<number>(0)
  const durationIntervalRef = useRef<number>(0)

  const startRecording = async () => {
    try {
      const preListener = new MicrophoneListener(threshold)
      const postListener = new PostambleDetector(threshold)
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true })

      const audioContext = new (window.AudioContext || (window as any).webkitAudioContext)()
      const source = audioContext.createMediaStreamSource(stream)
      const processor = audioContext.createScriptProcessor(4096, 1, 1)

      sourceRef.current = source
      processorRef.current = processor
      streamRef.current = stream

      source.connect(processor)
      processor.connect(audioContext.destination)

      setIsRecording(true)
      recordedSamplesRef.current = []
      setDecodedText(null)
      startTimeRef.current = Date.now()

      setStatus('Waiting for preamble...')
      setStatusType('info')

      let recordingState: 'waiting_preamble' | 'waiting_postamble' | 'complete' = 'waiting_preamble'
      let dataStart = 0
      let dataEnd = 0

      // Update duration every second
      durationIntervalRef.current = window.setInterval(() => {
        const elapsed = Math.floor((Date.now() - startTimeRef.current) / 1000)
        setDuration(elapsed)
      }, 100)

      processor.onaudioprocess = (event: AudioProcessingEvent) => {
        const audioSamples = Array.from(event.inputData.getChannelData(0))
        recordedSamplesRef.current.push(...audioSamples)
        setSamples(recordedSamplesRef.current.length)

        if (recordingState === 'waiting_preamble') {
          const pos = preListener.add_samples(audioSamples)
          if (pos >= 0) {
            dataStart = recordedSamplesRef.current.length
            recordingState = 'waiting_postamble'
            setStatus('Preamble detected! Waiting for postamble...')
            setStatusType('success')
          }
        } else if (recordingState === 'waiting_postamble') {
          const pos = postListener.add_samples(audioSamples)
          if (pos >= 0) {
            dataEnd = recordedSamplesRef.current.length - postListener.buffer_size()
            recordingState = 'complete'
            setStatus('Postamble detected! Decoding...')
            setStatusType('success')

            // Stop recording and process
            stopRecording()
            processDecode(
              recordedSamplesRef.current.slice(dataStart, dataEnd),
              audioContext.sampleRate
            )
          }
        }
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to start recording'
      setStatus(`Error: ${message}`)
      setStatusType('error')
    }
  }

  const stopRecording = () => {
    if (processorRef.current && sourceRef.current && streamRef.current) {
      processorRef.current.disconnect()
      sourceRef.current.disconnect()
      streamRef.current.getTracks().forEach((track) => track.stop())
    }

    if (durationIntervalRef.current) {
      clearInterval(durationIntervalRef.current)
    }

    setIsRecording(false)
  }

  const processDecode = async (samplesData: number[], sampleRate: number) => {
    try {
      let processedSamples = samplesData
      if (sampleRate !== 16000) {
        processedSamples = resampleAudio(samplesData, sampleRate, 16000)
      }

      const decoder = await createDecoder()
      const data = await decoder.decode(processedSamples)
      const text = new TextDecoder().decode(data)

      setDecodedText(text)
      setStatus('Decoded successfully!')
      setStatusType('success')
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Decode failed'
      setStatus(`Decode failed: ${message}`)
      setStatusType('error')
    }
  }

  return (
    <div className="container">
      <div className="text-center mb-5">
        <h1>🎙️ Live Recording & Decode</h1>
        <p>Record from microphone, detect preamble/postamble, and decode the message</p>
      </div>

      <div className="card">
        <h2>Recording Settings</h2>

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
              disabled={isRecording}
            />
            <span>{threshold.toFixed(1)}</span>
          </div>
        </div>

        <div className="mt-4">
          <button
            onClick={startRecording}
            disabled={isRecording}
            className="btn-primary w-full"
          >
            Start Recording
          </button>
          {isRecording && (
            <button
              onClick={stopRecording}
              className="btn-secondary w-full mt-3"
            >
              Stop Recording
            </button>
          )}
        </div>

        {status && <Status message={status} type={statusType} />}

        {isRecording && (
          <div className="mt-4">
            <p><strong>Recording Status:</strong></p>
            <div style={{ background: '#f7fafc', padding: '1rem', borderRadius: '0.5rem' }}>
              <div>Duration: {duration}s</div>
              <div>Samples: {samples}</div>
            </div>
          </div>
        )}

        {decodedText !== null && (
          <div className="mt-4">
            <p><strong>Decoded Message:</strong></p>
            <div
              style={{
                background: '#f7fafc',
                padding: '1rem',
                borderRadius: '0.5rem',
                wordBreak: 'break-word',
                fontFamily: 'monospace',
                minHeight: '80px',
              }}
            >
              {decodedText}
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

export default RecordingDecodePage
