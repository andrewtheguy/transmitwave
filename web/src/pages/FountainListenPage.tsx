import React, { useState, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { PreambleDetector, createFountainDecoder } from '../utils/wasm'
import { resampleAudio } from '../utils/audio'
import Status from '../components/Status'
import { getMicProcessorUrl } from '../utils/mic-processor-inline'

const TARGET_SAMPLE_RATE = 16000
const TIMEOUT_SECS = 30
const BLOCK_SIZE = 64
const DETECTION_THRESHOLD = 0.4
const MAX_BUFFER_SAMPLES = 80000

const FountainListenPage: React.FC = () => {
  const navigate = useNavigate()

  const [isListening, setIsListening] = useState(false)
  const [isRecording, setIsRecording] = useState(false)
  const [status, setStatus] = useState<string | null>(null)
  const [statusType, setStatusType] = useState<'success' | 'error' | 'info' | 'warning'>('info')
  const [elapsed, setElapsed] = useState(0)
  const [decodedText, setDecodedText] = useState<string | null>(null)
  const [isDecoding, setIsDecoding] = useState(false)

  const processorRef = useRef<AudioWorkletNode | null>(null)
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const detectorRef = useRef<PreambleDetector | null>(null)
  const resampleBufferRef = useRef<number[]>([])
  const allResampledSamplesRef = useRef<number[]>([])
  const recordedSamplesRef = useRef<number[]>([])
  const recordingResampleBufferRef = useRef<number[]>([])
  const preambleDetectedRef = useRef<boolean>(false)
  const isRecordingRef = useRef<boolean>(false)
  const startTimeRef = useRef<number>(0)
  const timerIntervalRef = useRef<number | null>(null)
  const samplesProcessedRef = useRef<number>(0)

  const startListening = async () => {
    try {
      const detector = new PreambleDetector(DETECTION_THRESHOLD)
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

      source.connect(processor)
      processor.connect(audioContext.destination)

      setIsListening(true)
      setStatus('Listening for preamble...')
      setStatusType('info')
      setDecodedText(null)
      setElapsed(0)

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

          const position = detector.add_samples(new Float32Array(resampledChunk))
          samplesProcessedRef.current += resampledChunk.length

          if (samplesProcessedRef.current > MAX_BUFFER_SAMPLES) {
            detector.clear()
            samplesProcessedRef.current = 0
            resampleBufferRef.current = []
            allResampledSamplesRef.current = []
          }

          if (position >= 0) {
            preambleDetectedRef.current = true
            isRecordingRef.current = true
            setIsRecording(true)
            setStatus('Preamble detected! Recording fountain stream...')
            setStatusType('success')
            startTimeRef.current = Date.now()

            recordedSamplesRef.current = allResampledSamplesRef.current
            allResampledSamplesRef.current = []
            resampleBufferRef.current = []

            timerIntervalRef.current = window.setInterval(() => {
              const elapsedSecs = (Date.now() - startTimeRef.current) / 1000
              setElapsed(elapsedSecs)

              if (elapsedSecs >= TIMEOUT_SECS) {
                stopRecording('Recording complete (30 seconds)')
              }
            }, 100)

            detector.clear()
            samplesProcessedRef.current = 0
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

          recordedSamplesRef.current.push(...resampledChunk)
        }
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to access microphone'
      setStatus(`Error: ${message}`)
      setStatusType('error')
    }
  }

  const stopListening = () => {
    cleanup()
    setIsListening(false)
    setIsRecording(false)
    setStatus('Stopped listening')
    setStatusType('info')
  }

  const stopRecording = (message?: string) => {
    cleanup()
    isRecordingRef.current = false
    setIsRecording(false)
    setIsListening(false)

    if (message) {
      setStatus(message)
      setStatusType('success')
      setTimeout(() => {
        decodeFountainAudio()
      }, 100)
    }
  }

  const cleanup = () => {
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

    if (timerIntervalRef.current) {
      clearInterval(timerIntervalRef.current)
      timerIntervalRef.current = null
    }
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

      const decoder = await createFountainDecoder()
      const data = decoder.decode_fountain(
        new Float32Array(recordedSamplesRef.current),
        TIMEOUT_SECS,
        BLOCK_SIZE
      )
      const text = new TextDecoder().decode(data)

      setDecodedText(text)
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

  const resetAndListenAgain = () => {
    preambleDetectedRef.current = false
    isRecordingRef.current = false
    recordedSamplesRef.current = []
    recordingResampleBufferRef.current = []
    setDecodedText(null)
    setElapsed(0)
    setStatus(null)
    startListening()
  }

  const progressPercent = (elapsed / TIMEOUT_SECS) * 100

  return (
    <div className="container">
      <div className="text-center mb-5">
        <h1>Fountain Code Listener</h1>
        <p>Listen for 30 seconds of fountain-coded audio stream and decode</p>
      </div>

      <div className="card" style={{ maxWidth: '600px', margin: '0 auto' }}>
        <h2>Listening Controls</h2>

        <div className="mt-4">
          <button
            onClick={startListening}
            disabled={isListening}
            className="btn-primary w-full"
          >
            Start Listening
          </button>
        </div>

        {isListening && (
          <div className="mt-4">
            <button onClick={stopListening} className="btn-secondary w-full">
              Stop
            </button>
          </div>
        )}

        {status && <Status message={status} type={statusType} />}

        {isRecording && (
          <div className="mt-4">
            <div style={{ marginBottom: '0.5rem', display: 'flex', justifyContent: 'space-between' }}>
              <span><strong>Progress:</strong></span>
              <span>{elapsed.toFixed(1)}s / {TIMEOUT_SECS}s</span>
            </div>
            <div style={{
              width: '100%',
              height: '8px',
              background: '#e2e8f0',
              borderRadius: '4px',
              overflow: 'hidden'
            }}>
              <div style={{
                width: `${progressPercent}%`,
                height: '100%',
                background: '#4299e1',
                transition: 'width 0.1s linear'
              }} />
            </div>
          </div>
        )}

        <div className="mt-4" style={{ padding: '1rem', background: '#f7fafc', borderRadius: '0.5rem', fontSize: '0.9rem' }}>
          <p><strong>Configuration:</strong></p>
          <ul style={{ marginTop: '0.5rem', paddingLeft: '1.5rem' }}>
            <li>Duration: {TIMEOUT_SECS} seconds</li>
            <li>Block size: {BLOCK_SIZE} bytes</li>
            <li>Detection threshold: {DETECTION_THRESHOLD}</li>
          </ul>
        </div>
      </div>

      {!isListening && !isRecording && decodedText && (
        <div className="card" style={{ maxWidth: '600px', margin: '2rem auto 0' }}>
          <h2>Decoded Result</h2>

          <div style={{
            background: '#f7fafc',
            padding: '1rem',
            borderRadius: '0.5rem',
            wordBreak: 'break-word',
            fontFamily: 'monospace',
            minHeight: '80px',
            marginTop: '1rem'
          }}>
            {decodedText}
          </div>

          <button
            onClick={resetAndListenAgain}
            className="btn-primary w-full mt-4"
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
