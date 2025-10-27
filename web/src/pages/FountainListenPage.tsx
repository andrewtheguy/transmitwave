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
  const [hasRecording, setHasRecording] = useState(false)
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
      setHasRecording(false)

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
    // Flush any remaining samples in the recording buffer
    if (recordingResampleBufferRef.current.length > 0) {
      const actualSampleRate = audioContextRef.current?.sampleRate || 48000
      let resampledChunk = recordingResampleBufferRef.current
      if (actualSampleRate !== TARGET_SAMPLE_RATE) {
        resampledChunk = resampleAudio(recordingResampleBufferRef.current, actualSampleRate, TARGET_SAMPLE_RATE)
      }
      recordedSamplesRef.current.push(...resampledChunk)
      recordingResampleBufferRef.current = []
    }

    cleanup()
    isRecordingRef.current = false
    setIsRecording(false)
    setIsListening(false)

    const hasRecordedAudio = recordedSamplesRef.current.length > 0
    setHasRecording(hasRecordedAudio)
    console.log('Stop recording - samples:', recordedSamplesRef.current.length, 'hasRecording:', hasRecordedAudio)

    if (message) {
      setStatus(message)
      setStatusType('success')
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

      const samples = new Float32Array(recordedSamplesRef.current)
      console.log('Decoding with:', {
        sampleCount: samples.length,
        timeoutSecs: TIMEOUT_SECS,
        blockSize: BLOCK_SIZE,
        sampleRate: TARGET_SAMPLE_RATE,
        firstSamples: Array.from(samples.slice(0, 10)),
        hasNaN: samples.some(s => isNaN(s)),
        hasInfinity: samples.some(s => !isFinite(s))
      })

      const decoder = await createFountainDecoder()
      const data = decoder.decode_fountain(
        samples,
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

    try {
      setStatus('Loading WAV file...')
      setStatusType('info')

      const arrayBuffer = await file.arrayBuffer()
      const audioContext = new (window.AudioContext || (window as any).webkitAudioContext)()
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
    setHasRecording(false)
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

        {isListening && !isRecording && (
          <div className="mt-4">
            <button onClick={stopListening} className="btn-secondary w-full">
              Stop Listening
            </button>
          </div>
        )}

        {isRecording && (
          <div className="mt-4">
            <button onClick={() => stopRecording('Recording stopped manually')} className="btn-secondary w-full">
              Stop Recording
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
