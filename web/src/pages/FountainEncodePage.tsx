import React, { useState, useRef, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { createFountainEncoder } from '../utils/wasm'
import { createWavBlob } from '../utils/audio'
import Status from '../components/Status'

const FountainEncodePage: React.FC = () => {
  const navigate = useNavigate()
  const [text, setText] = useState('Hello fountain mode!')
  const [isEncoding, setIsEncoding] = useState(false)
  const [isPlaying, setIsPlaying] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const audioRef = useRef<HTMLAudioElement>(null)
  const downloadRef = useRef<HTMLAnchorElement>(null)
  const [audioUrl, setAudioUrl] = useState<string | null>(null)

  const TIMEOUT_SECS = 30
  const BLOCK_SIZE = 16
  const REPAIR_RATIO = 0.5

  useEffect(() => {
    return () => {
      if (audioUrl) {
        URL.revokeObjectURL(audioUrl)
      }
    }
  }, [audioUrl])

  const handleEncodeAndPlay = async () => {
    if (!text) {
      setError('Please enter text to encode')
      return
    }

    setIsEncoding(true)
    setError(null)

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
        <p>Continuous streaming mode using RaptorQ fountain codes - transmits for 30 seconds</p>
      </div>

      <div className="card" style={{ maxWidth: '600px', margin: '0 auto' }}>
        <h2>Encode & Stream</h2>

        <div className="mt-3">
          <label><strong>Message</strong></label>
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="Enter text to encode..."
            maxLength={200}
            disabled={isEncoding || isPlaying}
            style={{ minHeight: '120px', resize: 'vertical' }}
          />
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: '0.5rem' }}>
            <div style={{ fontSize: '0.9rem', color: '#718096' }}>
              {text.length} / 200 characters
            </div>
            <button
              onClick={handleClearText}
              disabled={isEncoding || isPlaying || !text}
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
            disabled={isEncoding || isPlaying || !text}
            className="btn-primary w-full"
          >
            {isEncoding ? 'Encoding...' : isPlaying ? 'Playing...' : 'Encode & Play'}
          </button>
        </div>

        {error && <Status message={error} type="error" />}

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
            <li>Repair ratio: {REPAIR_RATIO * 100}%</li>
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
