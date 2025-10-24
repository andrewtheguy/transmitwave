import { useState, useCallback } from 'react'
import { createDecoder, DecoderOptions } from '../utils/wasm'
import { parseWavFile, stereoToMono, resampleAudio } from '../utils/audio'

interface UseDecoderResult {
  decode: (file: File) => Promise<string | null>
  isDecoding: boolean
  error: string | null
}

export const useDecoder = (): UseDecoderResult => {
  const [isDecoding, setIsDecoding] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const decode = useCallback(async (file: File): Promise<string | null> => {
    setIsDecoding(true)
    setError(null)

    try {
      const buffer = await file.arrayBuffer()
      const wavData = parseWavFile(buffer)

      if (!wavData) {
        throw new Error('Invalid WAV file')
      }

      let samples = wavData.samples

      // Convert stereo to mono if needed
      if (wavData.channels > 1) {
        samples = stereoToMono(samples)
      }

      // Resample if needed
      if (wavData.sampleRate !== 16000) {
        samples = resampleAudio(samples, wavData.sampleRate, 16000)
      }

      const decoder = await createDecoder()
      const data = await decoder.decode(samples)
      const text = new TextDecoder().decode(data)

      return text
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Decoding failed'
      setError(message)
      return null
    } finally {
      setIsDecoding(false)
    }
  }, [])

  return { decode, isDecoding, error }
}
