import { useState, useCallback } from 'react'
import { createEncoder, EncoderOptions, WasmEncoder } from '../utils/wasm'
import { createWavBlob } from '../utils/audio'

interface UseEncoderResult {
  encode: (text: string, options?: EncoderOptions) => Promise<Blob | null>
  isEncoding: boolean
  error: string | null
}

export const useEncoder = (): UseEncoderResult => {
  const [isEncoding, setIsEncoding] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const encode = useCallback(async (text: string, options?: EncoderOptions): Promise<Blob | null> => {
    if (!text) {
      setError('Please enter text to encode')
      return null
    }

    setIsEncoding(true)
    setError(null)

    try {
      console.log('useEncoder: encode called with options:', options)
      const encoder = await createEncoder(options)
      console.log('useEncoder: encoder created, type:', encoder.constructor.name)
      const data = new TextEncoder().encode(text)
      const samples = encoder.encode(data)
      console.log('useEncoder: encoded', data.length, 'bytes to', samples.length, 'audio samples')

      const blob = createWavBlob(samples, 16000, 1)
      return blob
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
      setError(message)
      return null
    } finally {
      setIsEncoding(false)
    }
  }, [])

  return { encode, isEncoding, error }
}
