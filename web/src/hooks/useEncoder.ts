import { useState, useCallback } from 'react'
import { createEncoder, EncoderOptions, WasmEncoder, WasmEncoderLegacy, WasmEncoderSpread } from '../utils/wasm'
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
      const encoder = await createEncoder(options)
      const data = new TextEncoder().encode(text)
      const samples = await encoder.encode(data)

      const blob = createWavBlob(samples, 16000, 1)
      return blob
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Encoding failed'
      setError(message)
      return null
    } finally {
      setIsEncoding(false)
    }
  }, [])

  return { encode, isEncoding, error }
}
