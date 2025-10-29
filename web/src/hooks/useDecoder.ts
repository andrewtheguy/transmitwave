import { useState, useCallback } from 'react'
import { createDecoder, DecoderOptions } from '../utils/wasm'
import { parseWavFile, stereoToMono, resampleAudio } from '../utils/audio'

interface UseDecoderResult {
  decode: (file: File, options?: DecoderOptions) => Promise<string | null>
  decodeWithoutSync: (file: File, options?: DecoderOptions) => Promise<string | null>
  isDecoding: boolean
  error: string | null
}

export const useDecoder = (): UseDecoderResult => {
  const [isDecoding, setIsDecoding] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const decode = useCallback(async (file: File, options?: DecoderOptions): Promise<string | null> => {
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

      const decoder = await createDecoder(options)
      const data = await decoder.decode(samples)
      const text = new TextDecoder().decode(data)

      return text
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
      setError(message)
      return null
    } finally {
      setIsDecoding(false)
    }
  }, [])

  const decodeWithoutSync = useCallback(async (file: File, options?: DecoderOptions): Promise<string | null> => {
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

      // Extract FSK data without preamble/postamble to avoid double-detection
      // Preamble is 250ms at 16kHz = 4000 samples, plus 50ms silence = 800 samples
      const PREAMBLE_DURATION_MS = 250
      const PREAMBLE_SAMPLES = (16000 * PREAMBLE_DURATION_MS) / 1000 // 4000 samples
      const SYNC_SILENCE_SAMPLES = 800

      const dataStart = Math.min(PREAMBLE_SAMPLES + SYNC_SILENCE_SAMPLES, samples.length)
      const postambleEstimate = PREAMBLE_SAMPLES // Postamble is typically same duration as preamble
      const dataEnd = Math.max(dataStart, samples.length - postambleEstimate)

      // Extract only FSK data region
      const fskDataOnly = samples.slice(dataStart, dataEnd)

      if (fskDataOnly.length === 0) {
        throw new Error('Unable to extract FSK data from audio file')
      }

      const decoder = await createDecoder(options)
      const data = await decoder.decode_without_preamble_postamble(fskDataOnly)
      const text = new TextDecoder().decode(data)

      return text
    } catch (err) {
      let message = 'Decoding without sync failed'

      if (err instanceof Error) {
        message = err.message
      } else if (typeof err === 'string') {
        message = err
      } else if (err && typeof err === 'object' && 'message' in err) {
        message = String((err as any).message)
      }

      console.error('Decode without sync error details:', err)
      setError(message)
      return null
    } finally {
      setIsDecoding(false)
    }
  }, [])

  return { decode, decodeWithoutSync, isDecoding, error }
}
