import React, { useState, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { createEncoderDtmf, createDecoderDtmf } from '../utils/wasm'
import { createWavBlob, resampleAudio, stereoToMono } from '../utils/audio'
import Status from '../components/Status'

const DTMFPage: React.FC = () => {
  const navigate = useNavigate()

  // Encode section state
  const [encodeText, setEncodeText] = useState('Hello DTMF')
  const [isEncoding, setIsEncoding] = useState(false)
  const [encodeError, setEncodeError] = useState<string | null>(null)
  const [audioUrl, setAudioUrl] = useState<string | null>(null)

  // Decode section state
  const [decodeFile, setDecodeFile] = useState<File | null>(null)
  const [isDecoding, setIsDecoding] = useState(false)
  const [decodeError, setDecodeError] = useState<string | null>(null)
  const [decodedText, setDecodedText] = useState<string | null>(null)
  const [decodeStatus, setDecodeStatus] = useState<string | null>(null)
  const [decodeStatusType, setDecodeStatusType] = useState<'success' | 'error' | 'info' | 'warning'>('info')

  // Threshold settings
  const [preambleThreshold, setPreambleThreshold] = useState(0.4)
  const [postambleThreshold, setPostambleThreshold] = useState(0.4)

  const audioRef = useRef<HTMLAudioElement>(null)
  const downloadRef = useRef<HTMLAnchorElement>(null)

  const handleEncode = async () => {
    if (!encodeText) {
      setEncodeError('Please enter text to encode')
      return
    }

    setIsEncoding(true)
    setEncodeError(null)

    try {
      const encoder = await createEncoderDtmf()
      const data = new TextEncoder().encode(encodeText)
      const samples = encoder.encode(data)

      const blob = createWavBlob(samples, 16000, 1)
      const url = URL.createObjectURL(blob)
      setAudioUrl(url)
      setDecodedText(null)
      setEncodeError(null)
    } catch (err) {
      let message = 'Encoding failed'

      if (err instanceof Error) {
        message = err.message
      } else if (typeof err === 'string') {
        message = err
      } else if (err && typeof err === 'object' && 'message' in err) {
        message = String((err as any).message)
      }

      console.error('Encode error details:', err)
      setEncodeError(message)
    } finally {
      setIsEncoding(false)
    }
  }

  const handleDecode = async () => {
    if (!decodeFile) {
      setDecodeStatus('No file selected')
      setDecodeStatusType('error')
      return
    }

    setIsDecoding(true)
    setDecodeStatus('Decoding audio...')
    setDecodeStatusType('info')
    setDecodeError(null)

    try {
      // Read WAV file
      const arrayBuffer = await decodeFile.arrayBuffer()
      const wavData = new DataView(arrayBuffer)

      // Parse WAV header
      const riff = String.fromCharCode(wavData.getUint8(0), wavData.getUint8(1), wavData.getUint8(2), wavData.getUint8(3))
      if (riff !== 'RIFF') {
        throw new Error('Invalid WAV file format')
      }

      const sampleRate = wavData.getUint32(24, true)
      const bitsPerSample = wavData.getUint16(34, true)
      const numChannels = wavData.getUint16(22, true)
      const dataOffset = 44

      // Read audio samples
      const numSamples = (arrayBuffer.byteLength - dataOffset) / (bitsPerSample / 8) / numChannels
      let samples: number[] = []

      for (let i = 0; i < numSamples; i++) {
        const offset = dataOffset + i * numChannels * (bitsPerSample / 8)
        let sample: number

        if (bitsPerSample === 16) {
          sample = wavData.getInt16(offset, true) / 32768.0
        } else if (bitsPerSample === 32) {
          sample = wavData.getFloat32(offset, true)
        } else {
          throw new Error(`Unsupported bit depth: ${bitsPerSample}`)
        }

        samples.push(sample)
      }

      // Convert stereo to mono if needed
      if (numChannels === 2) {
        samples = stereoToMono(samples)
      }

      // Resample to 16kHz if needed
      if (sampleRate !== 16000) {
        samples = resampleAudio(samples, sampleRate, 16000)
      }

      // Create decoder with threshold settings
      const decoder = await createDecoderDtmf({
        preambleThreshold,
        postambleThreshold,
      })

      // Decode samples
      const decodedData = decoder.decode(new Float32Array(samples))
      const text = new TextDecoder().decode(decodedData)

      setDecodedText(text)
      setDecodeStatus(`Decoded successfully: "${text}"`)
      setDecodeStatusType('success')
      setDecodeError(null)
    } catch (err) {
      let message = 'Decoding failed'

      if (err instanceof Error) {
        message = err.message
      } else if (typeof err === 'string') {
        message = err
      } else if (err && typeof err === 'object' && 'message' in err) {
        message = String((err as any).message)
      }

      console.error('Decode error details:', err)
      setDecodeStatus(message)
      setDecodeStatusType('error')
      setDecodeError(message)
    } finally {
      setIsDecoding(false)
    }
  }

  const handleDownload = () => {
    if (audioUrl && downloadRef.current) {
      downloadRef.current.href = audioUrl
      downloadRef.current.download = 'dtmf-encoded-audio.wav'
      downloadRef.current.click()
    }
  }

  return (
    <div className="container">
      <div className="text-center mb-5">
        <h1>📞 DTMF Audio Modem</h1>
        <p>Encode text to audio and decode audio back to text using DTMF dual-tone signaling</p>
        <small style={{ color: '#718096' }}>Uses 48-symbol extended DTMF with Reed-Solomon FEC for reliable over-the-air transmission</small>
      </div>

      <div className="grid" style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))', gap: '2rem', marginBottom: '2rem' }}>
        {/* Encode Section */}
        <div className="card">
          <h2>📝 Encode Text to DTMF Audio</h2>

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
            <button
              onClick={handleEncode}
              disabled={isEncoding || !encodeText}
              className="btn-primary w-full"
            >
              {isEncoding ? 'Encoding...' : 'Encode to DTMF Audio'}
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
          <h2>🔊 Decode DTMF Audio to Text</h2>

          <div className="mt-3">
            <label><strong>Detection Thresholds</strong></label>
            <div className="mt-2">
              <label style={{ fontSize: '0.9rem' }}>Preamble Threshold: {preambleThreshold.toFixed(2)}</label>
              <input
                type="range"
                min="0.1"
                max="0.9"
                step="0.05"
                value={preambleThreshold}
                onChange={(e) => setPreambleThreshold(parseFloat(e.target.value))}
                style={{ width: '100%' }}
              />
            </div>
            <div className="mt-2">
              <label style={{ fontSize: '0.9rem' }}>Postamble Threshold: {postambleThreshold.toFixed(2)}</label>
              <input
                type="range"
                min="0.1"
                max="0.9"
                step="0.05"
                value={postambleThreshold}
                onChange={(e) => setPostambleThreshold(parseFloat(e.target.value))}
                style={{ width: '100%' }}
              />
            </div>
            <small style={{ color: '#718096' }}>Lower values = more sensitive. Default: 0.4</small>
          </div>

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
            <button
              onClick={handleDecode}
              disabled={isDecoding || !decodeFile}
              className="btn-primary w-full"
            >
              {isDecoding ? 'Decoding...' : 'Decode DTMF Audio'}
            </button>
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

export default DTMFPage
