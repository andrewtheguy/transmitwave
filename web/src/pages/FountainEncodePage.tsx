import React, { useState, useRef, useEffect, useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { createFountainEncoder } from '../utils/wasm'
import { createWavBlob } from '../utils/audio'
import Status from '../components/Status'
import { FOUNTAIN_BLOCK_SIZE_BYTES, MAX_PAYLOAD_BYTES } from '../constants/fountain'

const FountainEncodePage: React.FC = () => {
  const navigate = useNavigate()
  const [text, setText] = useState('Hello fountain mode!')
  const [isEncoding, setIsEncoding] = useState(false)
  const [isPlaying, setIsPlaying] = useState(false)
  const [isStreaming, setIsStreaming] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const audioRef = useRef<HTMLAudioElement>(null)
  const downloadRef = useRef<HTMLAnchorElement>(null)
  const [audioUrl, setAudioUrl] = useState<string | null>(null)
  const streamEncoderRef = useRef<Awaited<ReturnType<typeof createFountainEncoder>> | null>(null)
  const streamAudioContextRef = useRef<AudioContext | null>(null)
  const streamTimerRef = useRef<number | null>(null)
  const streamScheduledTimeRef = useRef(0)
  const isStreamingRef = useRef(false)

  const TIMEOUT_SECS = 30
  const BLOCK_SIZE = FOUNTAIN_BLOCK_SIZE_BYTES
  const REPAIR_RATIO = 0.5
  const STREAM_SAMPLE_RATE = 16000

  const stopStreaming = useCallback(() => {
    if (streamTimerRef.current !== null) {
      window.clearTimeout(streamTimerRef.current)
      streamTimerRef.current = null
    }

    try {
      streamEncoderRef.current?.stop_streaming()
    } catch (err) {
      console.warn('Error stopping fountain stream:', err)
    }
    streamEncoderRef.current = null

    if (streamAudioContextRef.current) {
      const ctx = streamAudioContextRef.current
      streamAudioContextRef.current = null
      ctx.close().catch((error) => {
        console.warn('Error closing AudioContext:', error)
      })
    }

    streamScheduledTimeRef.current = 0
    isStreamingRef.current = false
    setIsStreaming(false)
    setIsPlaying(false)
  }, [])

  useEffect(() => {
    return () => {
      if (audioUrl) {
        URL.revokeObjectURL(audioUrl)
      }
    }
  }, [audioUrl])

  useEffect(() => {
    return () => {
      stopStreaming()
    }
  }, [stopStreaming])

  const handleEncodeAndPlay = async () => {
    if (!text) {
      setError('Please enter text to encode')
      return
    }

    setIsEncoding(true)
    setError(null)
    stopStreaming()

    try {
      const encoder = await createFountainEncoder()
      const data = new TextEncoder().encode(text)
      const samples = encoder.encode_fountain(data, TIMEOUT_SECS, BLOCK_SIZE, REPAIR_RATIO)

      const blob = createWavBlob(samples, 16000, 1)
      const url = URL.createObjectURL(blob)

      if (audioUrl) {
        URL.revokeObjectURL(audioUrl)
      }
      setAudioUrl(url)

      setTimeout(() => {
        if (audioRef.current) {
          audioRef.current.play()
            .then(() => {
              setIsPlaying(true)
            })
            .catch((err) => {
              console.error('Play error:', err)
              setError('Failed to play audio: ' + err.message)
            })
        }
      }, 200)
    } catch (err) {
      let message = 'Encoding failed'
      if (err instanceof Error) {
        message = err.message
      } else if (typeof err === 'string') {
        message = err
      }
      console.error('Encode error:', err)
      setError(message)
    } finally {
      setIsEncoding(false)
    }
  }

  const scheduleStreamingBlock = useCallback((samples: Float32Array) => {
    const audioContext = streamAudioContextRef.current
    if (!audioContext || samples.length === 0) {
      return
    }

    const buffer = audioContext.createBuffer(1, samples.length, STREAM_SAMPLE_RATE)
    buffer.copyToChannel(samples, 0)

    const source = audioContext.createBufferSource()
    source.buffer = buffer
    source.connect(audioContext.destination)

    const now = audioContext.currentTime
    const nextStart = streamScheduledTimeRef.current > now
      ? streamScheduledTimeRef.current
      : now
    source.start(nextStart)

    const duration = samples.length / STREAM_SAMPLE_RATE
    streamScheduledTimeRef.current = nextStart + duration

    source.onended = () => {
      source.disconnect()
    }
  }, [STREAM_SAMPLE_RATE])

  const ensureStreamingBuffer = useCallback(() => {
    if (!isStreamingRef.current) {
      return
    }

    const encoder = streamEncoderRef.current
    const audioContext = streamAudioContextRef.current

    if (!encoder || !audioContext) {
      stopStreaming()
      return
    }

    const MIN_BUFFER_SECONDS = 1

    try {
      while (streamScheduledTimeRef.current - audioContext.currentTime < MIN_BUFFER_SECONDS) {
        const block = encoder.next_stream_block()
        if (!block || block.length === 0) {
          setError('Streaming ended (timeout reached)')
          stopStreaming()
          return
        }
        scheduleStreamingBlock(block)
      }
    } catch (err) {
      console.error('Streaming error:', err)
      const message = err instanceof Error ? err.message : 'Streaming failed'
      setError(message)
      stopStreaming()
      return
    }

    if (streamTimerRef.current !== null) {
      window.clearTimeout(streamTimerRef.current)
    }
    if (isStreamingRef.current) {
      streamTimerRef.current = window.setTimeout(ensureStreamingBuffer, 200)
    }
  }, [scheduleStreamingBlock, stopStreaming])

  const handleStartStreaming = useCallback(async () => {
    if (isStreaming) {
      return
    }

    if (!text) {
      setError('Please enter text to encode')
      return
    }

    setError(null)
    stopStreaming()

    try {
      const encoder = await createFountainEncoder()
      const data = new TextEncoder().encode(text)
      await encoder.start_streaming(data, BLOCK_SIZE, REPAIR_RATIO, 0)

      const AudioContextClass = (window.AudioContext || (window as any).webkitAudioContext)
      if (!AudioContextClass) {
        throw new Error('Web Audio API is not supported in this browser.')
      }

      const audioContext: AudioContext = new AudioContextClass()
      streamAudioContextRef.current = audioContext
      streamEncoderRef.current = encoder
      streamScheduledTimeRef.current = audioContext.currentTime
      isStreamingRef.current = true
      setIsStreaming(true)

      ensureStreamingBuffer()
    } catch (err) {
      console.error('Failed to start streaming:', err)
      const message = err instanceof Error ? err.message : 'Failed to start streaming'
      setError(message)
      stopStreaming()
    }
  }, [BLOCK_SIZE, REPAIR_RATIO, ensureStreamingBuffer, isStreaming, stopStreaming, text])

  const handleAudioEnded = () => {
    setIsPlaying(false)
  }

  const handleDownload = () => {
    if (audioUrl && downloadRef.current) {
      downloadRef.current.href = audioUrl
      downloadRef.current.download = 'fountain-encoded.wav'
      downloadRef.current.click()
    }
  }

  const handleClearText = () => {
    setText('')
    setError(null)
  }

  const handleReset = () => {
    stopStreaming()
    setText('Hello fountain mode!')
    setError(null)
    setIsPlaying(false)
    if (audioUrl) {
      URL.revokeObjectURL(audioUrl)
      setAudioUrl(null)
    }
    if (audioRef.current) {
      audioRef.current.pause()
      audioRef.current.currentTime = 0
    }
  }

  return (
    <div className="container">
      <div className="text-center mb-5">
        <h1>Fountain Code Encoder</h1>
        <p>Generate a timed clip or stream continuously using RaptorQ fountain codes. Stop the stream any time without buffering audio.</p>
      </div>

      <div className="card" style={{ maxWidth: '600px', margin: '0 auto' }}>
        <h2>Encode & Stream</h2>

        <div className="mt-3">
          <label><strong>Message</strong></label>
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="Enter text to encode..."
            maxLength={MAX_PAYLOAD_BYTES}
            disabled={isEncoding || isPlaying || isStreaming}
            style={{ minHeight: '120px', resize: 'vertical' }}
          />
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: '0.5rem' }}>
            <div style={{ fontSize: '0.9rem', color: '#718096' }}>
              {text.length} / {MAX_PAYLOAD_BYTES} characters
            </div>
            <button
              onClick={handleClearText}
              disabled={isEncoding || isPlaying || isStreaming || !text}
              className="btn-tertiary"
              style={{ fontSize: '0.85rem', padding: '0.4rem 0.8rem' }}
            >
              Clear
            </button>
          </div>
        </div>

        <div className="mt-4">
          <button
            onClick={handleEncodeAndPlay}
            disabled={isEncoding || isPlaying || isStreaming || !text}
            className="btn-primary w-full"
          >
            {isEncoding ? 'Encoding...' : isPlaying ? 'Playing...' : isStreaming ? 'Streaming...' : 'Encode & Play'}
          </button>
        </div>

        <div className="mt-3">
          <button
            onClick={handleStartStreaming}
            disabled={isEncoding || isPlaying || isStreaming || !text}
            className="btn-secondary w-full"
          >
            {isStreaming ? 'Streaming...' : 'Start Continuous Stream'}
          </button>
        </div>

        {isStreaming && (
          <div className="mt-2">
            <button
              onClick={stopStreaming}
              className="btn-secondary w-full"
              style={{ backgroundColor: '#e53e3e', color: '#fff' }}
            >
              Stop Streaming
            </button>
          </div>
        )}

        {error && <Status message={error} type="error" />}
        {isStreaming && !error && (
          <Status message="Streaming fountain blocks... audio is generated live and not buffered." type="info" />
        )}

        {audioUrl && (
          <div className="mt-4">
            <p><strong>Audio:</strong></p>
            <audio
              ref={audioRef}
              controls
              style={{ width: '100%' }}
              onEnded={handleAudioEnded}
              src={audioUrl}
            />
            <div style={{ display: 'flex', gap: '0.5rem', marginTop: '1rem' }}>
              <button onClick={handleDownload} className="btn-secondary" style={{ flex: 1 }}>
                Download WAV
              </button>
              <button onClick={handleReset} className="btn-secondary" style={{ flex: 1 }}>
                Reset
              </button>
            </div>
          </div>
        )}

        <div className="mt-4" style={{ padding: '1rem', background: '#f7fafc', borderRadius: '0.5rem', fontSize: '0.9rem' }}>
          <p><strong>Configuration:</strong></p>
          <ul style={{ marginTop: '0.5rem', paddingLeft: '1.5rem' }}>
            <li>Duration: {TIMEOUT_SECS} seconds</li>
            <li>Block size: {BLOCK_SIZE} bytes</li>
            <li>Max payload: {MAX_PAYLOAD_BYTES} bytes</li>
            <li>Repair ratio: {REPAIR_RATIO * 100}%</li>
            <li>Continuous stream: runs until manually stopped (no buffering)</li>
          </ul>
        </div>
      </div>

      <div className="text-center mt-4">
        <button onClick={() => navigate('/')} className="btn-secondary">
          ← Back to Home
        </button>
      </div>

      <a ref={downloadRef} style={{ display: 'none' }} />
    </div>
  )
}

export default FountainEncodePage
