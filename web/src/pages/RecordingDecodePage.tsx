import React, { useState, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { PreambleDetector, PostambleDetector, createDecoder } from '../utils/wasm'
import { resampleAudio } from '../utils/audio'
import Status from '../components/Status'

const MAX_DURATION = 30
const TARGET_SAMPLE_RATE = 16000

const RecordingDecodePage: React.FC = () => {
  const navigate = useNavigate()
  const [isRecording, setIsRecording] = useState(false)
  const [recordingStatus, setRecordingStatus] = useState<string | null>(null)
  const [recordingStatusType, setRecordingStatusType] = useState<'success' | 'error' | 'info' | 'warning'>('info')
  const [detectionStatus, setDetectionStatus] = useState<string | null>(null)
  const [detectionStatusType, setDetectionStatusType] = useState<'success' | 'error' | 'info' | 'warning'>('info')
  const [duration, setDuration] = useState(0)
  const [samples, setSamples] = useState(0)
  const [decodedText, setDecodedText] = useState<string | null>(null)
  const [isDetecting, setIsDetecting] = useState(false)
  const [preambleDetected, setPreambleDetected] = useState(false)
  const [postambleDetected, setPostambleDetected] = useState(false)

  const processorRef = useRef<ScriptProcessorNode | null>(null)
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null)
  const gainNodeRef = useRef<GainNode | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const recordedSamplesRef = useRef<number[]>([])
  const startTimeRef = useRef<number>(0)
  const durationIntervalRef = useRef<number>(0)
  const audioContextRef = useRef<AudioContext | null>(null)

  const startRecording = async () => {
    try {
      // Request audio with constraints to disable auto-gain control and noise suppression
      // These features can reduce volume mid-recording, breaking FSK detection
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
        } as any,
      })

      const ctx = new (window.AudioContext || (window as any).webkitAudioContext)()
      audioContextRef.current = ctx

      const source = ctx.createMediaStreamSource(stream)

      // Create a gain node to normalize microphone input volume
      const gainNode = ctx.createGain()
      gainNode.gain.value = 2.0 // Boost input by 2x to compensate for quiet mics

      const processor = ctx.createScriptProcessor(4096, 1, 1)

      sourceRef.current = source
      gainNodeRef.current = gainNode
      processorRef.current = processor
      streamRef.current = stream

      // Connect with gain node for volume normalization
      source.connect(gainNode)
      gainNode.connect(processor)
      processor.connect(ctx.destination)

      setIsRecording(true)
      recordedSamplesRef.current = []
      setDecodedText(null)
      setRecordingStatus('Recording...')
      setRecordingStatusType('info')
      startTimeRef.current = Date.now()

      // Update duration every 100ms
      durationIntervalRef.current = window.setInterval(() => {
        const elapsed = Math.floor((Date.now() - startTimeRef.current) / 1000)
        setDuration(elapsed)
        setSamples(recordedSamplesRef.current.length)

        // Auto-stop at 30 seconds
        if (elapsed >= MAX_DURATION) {
          stopRecording(`Recording stopped (max ${MAX_DURATION}s reached)`)
        }
      }, 100)

      processor.onaudioprocess = (event: AudioProcessingEvent) => {
        const audioSamples = Array.from((event as any).inputBuffer.getChannelData(0))

        // Normalize samples to prevent clipping while preserving quiet signals
        // Apply soft normalization to maintain dynamic range
        const normalizedSamples = audioSamples.map((sample) => {
          // Soft clipping to prevent distortion
          if (Math.abs(sample) > 1.0) {
            return Math.sign(sample) * (1.0 - Math.exp(-Math.abs(sample)))
          }
          return sample
        })

        recordedSamplesRef.current.push(...normalizedSamples)
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to start recording'
      setRecordingStatus(`Error: ${message}`)
      setRecordingStatusType('error')
    }
  }

  const stopRecording = (message?: string) => {
    // Disconnect audio nodes in reverse order
    if (processorRef.current) {
      processorRef.current.disconnect()
    }

    if (gainNodeRef.current) {
      gainNodeRef.current.disconnect()
    }

    if (sourceRef.current) {
      sourceRef.current.disconnect()
    }

    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop())
    }

    if (durationIntervalRef.current) {
      clearInterval(durationIntervalRef.current)
    }

    setIsRecording(false)
    // Only set message if provided (when called from auto-stop), otherwise clear it
    if (message) {
      setRecordingStatus(message)
      setRecordingStatusType('info')
    } else {
      setRecordingStatus(null)
    }
  }

  const processDetectAndDecode = async () => {
    if (recordedSamplesRef.current.length === 0) {
      setDetectionStatus('No audio recorded to detect')
      setDetectionStatusType('error')
      return
    }

    try {
      setIsDetecting(true)
      setDetectionStatus('Resampling to 16kHz...')
      setDetectionStatusType('info')

      const actualSampleRate = audioContextRef.current?.sampleRate || 48000
      let resampledSamples = recordedSamplesRef.current
      if (actualSampleRate !== TARGET_SAMPLE_RATE) {
        resampledSamples = resampleAudio(recordedSamplesRef.current, actualSampleRate, TARGET_SAMPLE_RATE)
      }

      // Detect preamble
      setDetectionStatus('Detecting preamble...')
      const preambleDetectorInst = new PreambleDetector(0.4)
      const preamblePos = preambleDetectorInst.add_samples(new Float32Array(resampledSamples))

      if (preamblePos === -1) {
        setDetectionStatus('Preamble not detected. Try adjusting threshold.')
        setDetectionStatusType('error')
        setIsDetecting(false)
        return
      }

      setPreambleDetected(true)
      setDetectionStatus('Preamble detected! Detecting postamble...')
      setDetectionStatusType('success')

      // Detect postamble
      const detector = new PostambleDetector(0.4)
      const postambleSearchStart = preamblePos + 8000
      if (postambleSearchStart >= resampledSamples.length) {
        setDetectionStatus('Not enough audio after preamble for postamble detection')
        setDetectionStatusType('error')
        setIsDetecting(false)
        return
      }

      const postambleSegment = resampledSamples.slice(postambleSearchStart)
      const postamblePos = detector.add_samples(new Float32Array(postambleSegment))

      if (postamblePos === -1) {
        setDetectionStatus('Postamble not detected. Try adjusting threshold.')
        setDetectionStatusType('error')
        setIsDetecting(false)
        return
      }

      setPostambleDetected(true)
      setDetectionStatus('Both detected! Decoding...')
      setDetectionStatusType('success')

      // Decode using full resampled audio
      const decoder = await createDecoder()
      const data = decoder.decode(new Float32Array(resampledSamples))
      const text = new TextDecoder().decode(data)

      setDecodedText(text)
      setDetectionStatus(`Decoded successfully: "${text}"`)
      setDetectionStatusType('success')
    } catch (error) {
      let message = 'Detection/decode failed'

      if (error instanceof Error) {
        message = error.message
      } else if (typeof error === 'string') {
        message = error
      } else if (error && typeof error === 'object' && 'message' in error) {
        message = String((error as any).message)
      }

      console.error('Detection/decode error details:', error)
      setDetectionStatus(message)
      setDetectionStatusType('error')
    } finally {
      setIsDetecting(false)
    }
  }

  const saveWave = () => {
    if (recordedSamplesRef.current.length === 0) {
      setRecordingStatus('No audio recorded to save')
      setRecordingStatusType('error')
      return
    }

    try {
      const actualSampleRate = audioContextRef.current?.sampleRate || 48000

      // Resample to 16kHz for standard WAV download
      let samplesToSave = recordedSamplesRef.current
      if (actualSampleRate !== TARGET_SAMPLE_RATE) {
        samplesToSave = resampleAudio(recordedSamplesRef.current, actualSampleRate, TARGET_SAMPLE_RATE)
      }

      // Convert to WAV
      const wav = encodeWAV(samplesToSave, TARGET_SAMPLE_RATE)
      const blob = new Blob([wav], { type: 'audio/wav' })
      const url = URL.createObjectURL(blob)

      // Trigger download
      const a = document.createElement('a')
      a.href = url
      a.download = `recording-${Date.now()}.wav`
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

  return (
    <div className="container">
      <div className="text-center mb-5">
        <h1>🎤 Live Recording & Decode</h1>
        <p>Record audio (max {MAX_DURATION}s), save WAV, or detect & decode with FSK (Four-Frequency Shift Keying)</p>
      </div>

      <div className="card">
        <h2>Recording Controls</h2>

        <div className="mt-4 flex gap-3">
          <button
            onClick={startRecording}
            disabled={isRecording}
            className="btn-primary w-full"
          >
            Start Recording
          </button>
          {isRecording && (
            <button
              onClick={() => stopRecording()}
              className="btn-secondary w-full"
            >
              Stop Recording
            </button>
          )}
        </div>

        {recordingStatus && <Status message={recordingStatus} type={recordingStatusType} />}

        {(isRecording || samples > 0) && (
          <div className="mt-4">
            <p><strong>Recording Status:</strong></p>
            <div style={{ background: '#f7fafc', padding: '1rem', borderRadius: '0.5rem' }}>
              <div>Duration: {duration}s / {MAX_DURATION}s</div>
              <div>Samples: {samples}</div>
            </div>
          </div>
        )}

        {!isRecording && samples > 0 && (
          <div className="mt-4 flex gap-3">
            <button
              onClick={saveWave}
              className="btn-secondary w-full"
            >
              💾 Download WAV
            </button>
            <button
              onClick={processDetectAndDecode}
              disabled={isDetecting}
              className="btn-primary w-full"
            >
              {isDetecting ? 'Detecting...' : '🔍 Detect & Decode'}
            </button>
          </div>
        )}
      </div>

      {(preambleDetected || postambleDetected || detectionStatus) && (
        <div className="card">
          <h2>Detection Status</h2>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '1rem', marginBottom: '1rem' }}>
            <div style={{ background: '#f7fafc', padding: '1rem', borderRadius: '0.5rem' }}>
              <div style={{ color: '#999', fontSize: '0.9rem', marginBottom: '0.5rem' }}>PREAMBLE</div>
              <div style={{ fontSize: '1.2rem', fontWeight: 'bold', color: preambleDetected ? '#48bb78' : '#999' }}>
                {preambleDetected ? '✓ Detected' : '○ Not detected'}
              </div>
            </div>
            <div style={{ background: '#f7fafc', padding: '1rem', borderRadius: '0.5rem' }}>
              <div style={{ color: '#999', fontSize: '0.9rem', marginBottom: '0.5rem' }}>POSTAMBLE</div>
              <div style={{ fontSize: '1.2rem', fontWeight: 'bold', color: postambleDetected ? '#48bb78' : '#999' }}>
                {postambleDetected ? '✓ Detected' : '○ Not detected'}
              </div>
            </div>
          </div>

          {detectionStatus && <Status message={detectionStatus} type={detectionStatusType} />}
        </div>
      )}

      {decodedText !== null && (
        <div className="card">
          <h2>Decoded Message</h2>
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

      <button onClick={() => navigate('/')} className="btn-secondary">
        ← Back to Home
      </button>
    </div>
  )
}

export default RecordingDecodePage
