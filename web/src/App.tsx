import React, { useEffect, useState } from 'react'
import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom'
import { initWasm } from './utils/wasm'
import Navigation from './components/Navigation'
import IndexPage from './pages/IndexPage'
import DemoPage from './pages/DemoPage'
import AmplePage from './pages/AmplePage'
import RecordingDecodePage from './pages/RecordingDecodePage'
import PreamblePostambleRecordPage from './pages/PreamblePostambleRecordPage'

const App: React.FC = () => {
  const [wasmReady, setWasmReady] = useState(false)
  const [wasmError, setWasmError] = useState<string | null>(null)

  useEffect(() => {
    initWasm()
      .then(() => {
        setWasmReady(true)
        console.log('WASM module initialized successfully')
      })
      .catch(error => {
        const message = error instanceof Error ? error.message : 'Unknown error'
        setWasmError(message)
        console.error('Failed to initialize WASM:', message)
      })
  }, [])

  if (wasmError) {
    return (
      <div className="container mt-5">
        <div className="card">
          <h1>Failed to Initialize</h1>
          <div className="status status-error">
            WASM Initialization Error: {wasmError}
          </div>
          <p>Please refresh the page to try again.</p>
        </div>
      </div>
    )
  }

  if (!wasmReady) {
    return (
      <div className="container mt-5">
        <div className="card text-center">
          <h1>Loading...</h1>
          <div className="spinner"></div>
          <p>Initializing audio modem...</p>
        </div>
      </div>
    )
  }

  return (
    <Router>
      <Navigation />
      <Routes>
        <Route path="/" element={<IndexPage />} />
        <Route path="/demo" element={<DemoPage />} />
        <Route path="/ample" element={<AmplePage />} />
        <Route path="/recording-decode" element={<RecordingDecodePage />} />
        <Route path="/preamble-postamble-record" element={<PreamblePostambleRecordPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Router>
  )
}

export default App
