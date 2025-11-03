import React, { useEffect, useState } from 'react'
import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom'
import { initWasm } from './utils/wasm'
import Navigation from './components/Navigation'
import IndexPage from './pages/IndexPage'
import StandardFskPage from './pages/StandardFskPage'
import SignalDetectionPage from './pages/SignalDetectionPage'
import StandardFskListenPage from './pages/StandardFskListenPage'
import FountainEncodePage from './pages/FountainEncodePage'
import FountainListenPage from './pages/FountainListenPage'

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
        <Route path="/standard-fsk" element={<StandardFskPage />} />
        <Route path="/signal-detection" element={<SignalDetectionPage />} />
        <Route path="/standard-fsk-listen" element={<StandardFskListenPage />} />
        <Route path="/fountain-encode" element={<FountainEncodePage />} />
        <Route path="/fountain-listen" element={<FountainListenPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Router>
  )
}

export default App
