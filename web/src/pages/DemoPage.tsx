import React, { useState, useRef, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { useEncoder } from '../hooks/useEncoder'
import { useDecoder } from '../hooks/useDecoder'
import Status from '../components/Status'

const DemoPage: React.FC = () => {
  const navigate = useNavigate()
  const { encode, isEncoding, error: encodeError } = useEncoder()
  const { decode, isDecoding, error: decodeError } = useDecoder()

  const [encodeText, setEncodeText] = useState('Hello World')
  const [audioUrl, setAudioUrl] = useState<string | null>(null)
  const [decodedText, setDecodedText] = useState<string | null>(null)
  const [decodeFile, setDecodeFile] = useState<File | null>(null)
  const [decodeStatus, setDecodeStatus] = useState<string | null>(null)
  const [decodeStatusType, setDecodeStatusType] = useState<'success' | 'error' | 'info' | 'warning'>('info')
  const [useChirp, setUseChirp] = useState(false)
  const audioRef = useRef<HTMLAudioElement>(null)
  const downloadRef = useRef<HTMLAnchorElement>(null)

  const handleEncode = async () => {
    const blob = await encode(encodeText, { useChirp })
    if (blob) {
      const url = URL.createObjectURL(blob)
      setAudioUrl(url)
      setDecodedText(null)
    }
  }

  const handleDecode = async () => {
    if (!decodeFile) {
      setDecodeStatus('No file selected')
      setDecodeStatusType('error')
      return
    }

    setDecodeStatus('Decoding audio...')
    setDecodeStatusType('info')

    try {
      // Demo page encodes and immediately decodes clean audio
      // Use automatic sync detection which handles both with and without postamble
      const text = await decode(decodeFile, { useChirp })

      if (text !== null) {
        setDecodedText(text)
        setDecodeStatus(`Decoded successfully: "${text}"`)
        setDecodeStatusType('success')
      } else if (decodeError) {
        // Error was set by the hook, show it
        setDecodeStatus(decodeError)
        setDecodeStatusType('error')
      }
    } catch (error) {
      let message = 'Decoding failed'

      if (error instanceof Error) {
        message = error.message
      } else if (typeof error === 'string') {
        message = error
      } else if (error && typeof error === 'object' && 'message' in error) {
        message = String((error as any).message)
      }

      console.error('Decode error details:', error)
      setDecodeStatus(message)
      setDecodeStatusType('error')
    }
  }

  const handleDownload = () => {
    if (audioUrl && downloadRef.current) {
      downloadRef.current.href = audioUrl
      downloadRef.current.download = 'encoded-audio.wav'
      downloadRef.current.click()
    }
  }

  return (
    <div className="container">
      <div className="text-center mb-5">
        <h1>Audio Modem Demo</h1>
        <p>Encode text to audio and decode audio back to text using FSK (Four-Frequency Shift Keying)</p>
      </div>

      <div className="grid" style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))', gap: '2rem', marginBottom: '2rem' }}>
        {/* Encode Section */}
        <div className="card">
          <h2>📝 Encode Text to Audio</h2>

          <div className="mt-3">
            <label><strong>Message</strong></label>
            <textarea
              value={encodeText}
              onChange={(e) => setEncodeText(e.target.value)}
              placeholder="Enter text to encode..."
              maxLength={200}
              style={{ minHeight: '120px', resize: 'vertical' }}
            />
            <div style={{ textAlign: 'right', marginTop: '0.5rem', fontSize: '0.9rem', color: '#718096' }}>
              {encodeText.length} / 200 characters
            </div>
          </div>

          <div className="mt-4">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={useChirp}
                onChange={(e) => setUseChirp(e.target.checked)}
              />
              <strong>Use Chirp FSK</strong>
            </label>
            <small style={{ color: '#718096' }}>
              {useChirp
                ? '✓ Better noise/multipath immunity (higher CPU)'
                : '○ Standard FSK (lower CPU)'}
            </small>
          </div>

          <div className="mt-4">
            <button
              onClick={handleEncode}
              disabled={isEncoding || !encodeText}
              className="btn-primary w-full"
            >
              {isEncoding ? 'Encoding...' : 'Encode to Audio'}
            </button>
          </div>

          {encodeError && <Status message={encodeError} type="error" />}

          {audioUrl && (
            <div className="mt-4">
              <p><strong>Encoded Audio:</strong></p>
              <audio ref={audioRef} controls style={{ width: '100%' }} src={audioUrl} />
              <button onClick={handleDownload} className="btn-secondary w-full mt-3">
                Download WAV
              </button>
            </div>
          )}
        </div>

        {/* Decode Section */}
        <div className="card">
          <h2>🔊 Decode Audio to Text</h2>

          <div className="mt-3">
            <label><strong>Upload WAV File</strong></label>
            <input
              type="file"
              accept=".wav,.mp3"
              onChange={(e) => {
                setDecodeFile(e.target.files?.[0] || null)
                setDecodeStatus(null)
                setDecodedText(null)
              }}
            />
          </div>

          <div className="mt-4">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={useChirp}
                onChange={(e) => setUseChirp(e.target.checked)}
              />
              <strong>Use Chirp FSK</strong>
            </label>
            <small style={{ color: '#718096' }}>
              ⚠️ Must match the encoding mode (if audio was encoded with chirp, decode with chirp)
            </small>
          </div>

          <div className="mt-4">
            <button
              onClick={handleDecode}
              disabled={isDecoding || !decodeFile}
              className="btn-primary w-full"
              title={useChirp ? "Uses automatic sync detection for chirp mode" : "Uses manual timing extraction for standard mode"}
            >
              {isDecoding ? 'Decoding...' : 'Decode Audio'}
            </button>
            {useChirp && (
              <small className="text-gray-500 mt-2 block">
                ℹ️ Using automatic preamble/postamble detection for chirp mode compatibility
              </small>
            )}
          </div>

          {decodeError && <Status message={decodeError} type="error" />}
          {decodeStatus && <Status message={decodeStatus} type={decodeStatusType} />}

          {decodedText !== null && (
            <div className="mt-4">
              <p><strong>Decoded Message:</strong></p>
              <div style={{
                background: '#f7fafc',
                padding: '1rem',
                borderRadius: '0.5rem',
                wordBreak: 'break-word',
                fontFamily: 'monospace',
                minHeight: '80px',
              }}>
                {decodedText}
              </div>
            </div>
          )}
        </div>
      </div>

      <button onClick={() => navigate('/')} className="btn-secondary">
        ← Back to Home
      </button>

      <a ref={downloadRef} style={{ display: 'none' }} />
    </div>
  )
}

export default DemoPage
