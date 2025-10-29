import React from 'react'
import { useNavigate } from 'react-router-dom'

const IndexPage: React.FC = () => {
  const navigate = useNavigate()

  return (
    <div className="container">
      <div className="text-center mb-5">
        <h1>transmitwave</h1>
        <p style={{ fontSize: '1.25rem', opacity: 0.9 }}>
          FSK Audio Modem for Reliable Over-the-Air Communication
        </p>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: '2rem', marginTop: '3rem' }}>
        <div
          className="card transition-all"
          style={{ cursor: 'pointer', height: '100%', border: '2px solid var(--primary-color)' }}
          onClick={() => navigate('/demo')}
        >
          <h3>📝 Main Demo</h3>
          <p>Encode text to audio and decode audio back to text with real-time WAV file generation.</p>
          <div style={{ fontSize: '0.85rem', color: '#059669', marginBottom: '1rem', fontWeight: 'bold' }}>✓ Recommended (Reed-Solomon)</div>
          <button className="btn-primary w-full">Open Demo</button>
        </div>

        <div
          className="card transition-all"
          style={{ cursor: 'pointer', height: '100%' }}
          onClick={() => navigate('/preamble-postamble-record')}
        >
          <h3>🎙️ Preamble → Record → Postamble</h3>
          <p>Listen for preamble to auto-start recording, auto-stop on postamble, then auto-decode message.</p>
          <div style={{ fontSize: '0.85rem', color: '#059669', marginBottom: '1rem', fontWeight: 'bold' }}>✓ Recommended (Reed-Solomon)</div>
          <button className="btn-primary w-full">Open Demo</button>
        </div>

        <div
          className="card transition-all"
          style={{ cursor: 'pointer', height: '100%' }}
          onClick={() => navigate('/ample')}
        >
          <h3>🎤 Signal Detection</h3>
          <p>Real-time detection of preamble and postamble synchronization signals from your microphone input.</p>
          <button className="btn-primary w-full">Open Demo</button>
        </div>

        <div
          className="card transition-all"
          style={{ cursor: 'pointer', height: '100%', opacity: 0.8, borderLeft: '4px solid #f59e0b' }}
          onClick={() => navigate('/fountain-encode')}
        >
          <h3>⛲ Fountain Encode</h3>
          <p>Encode and continuously stream data for 30 seconds using RaptorQ fountain codes.</p>
          <div style={{ fontSize: '0.85rem', color: '#f59e0b', marginBottom: '1rem', fontWeight: 'bold' }}>⚠️ Experimental - Not fully working</div>
          <button className="btn-primary w-full">Open Demo</button>
        </div>

        <div
          className="card transition-all"
          style={{ cursor: 'pointer', height: '100%', opacity: 0.8, borderLeft: '4px solid #f59e0b' }}
          onClick={() => navigate('/fountain-listen')}
        >
          <h3>🎧 Fountain Listen</h3>
          <p>Listen for fountain-coded stream and decode data after 30 seconds of reception.</p>
          <div style={{ fontSize: '0.85rem', color: '#f59e0b', marginBottom: '1rem', fontWeight: 'bold' }}>⚠️ Experimental - Not fully working</div>
          <button className="btn-primary w-full">Open Demo</button>
        </div>
      </div>

      <div className="card mt-5">
        <h2>Features</h2>
        <ul style={{ listStyle: 'none', padding: 0 }}>
          <li style={{ margin: '0.5rem 0' }}>✅ Multi-tone FSK modulation (6 simultaneous frequencies)</li>
          <li style={{ margin: '0.5rem 0' }}>✅ Reed-Solomon error correction (255, 223)</li>
          <li style={{ margin: '0.5rem 0' }}>✅ 400-2300 Hz sub-bass frequency band</li>
          <li style={{ margin: '0.5rem 0' }}>✅ 16 kHz sample rate</li>
          <li style={{ margin: '0.5rem 0' }}>✅ 200 byte max payload per transmission</li>
          <li style={{ margin: '0.5rem 0' }}>✅ Real-time preamble/postamble detection</li>
          <li style={{ margin: '0.5rem 0' }}>✅ Auto-gain adjustment for variable input levels</li>
          <li style={{ margin: '0.5rem 0' }}>✅ Optimized for over-the-air audio transmission</li>
        </ul>
      </div>

      <div className="card mt-5">
        <h2>Technical Specifications</h2>
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <tbody>
            <tr style={{ borderBottom: '1px solid var(--border-color)' }}>
              <td style={{ padding: '0.5rem' }}>
                <strong>Modulation</strong>
              </td>
              <td style={{ padding: '0.5rem' }}>Multi-tone FSK (6 tones/symbol)</td>
            </tr>
            <tr style={{ borderBottom: '1px solid var(--border-color)' }}>
              <td style={{ padding: '0.5rem' }}>
                <strong>Sample Rate</strong>
              </td>
              <td style={{ padding: '0.5rem' }}>16,000 Hz</td>
            </tr>
            <tr style={{ borderBottom: '1px solid var(--border-color)' }}>
              <td style={{ padding: '0.5rem' }}>
                <strong>Symbol Duration (Normal)</strong>
              </td>
              <td style={{ padding: '0.5rem' }}>192 ms (3072 samples)</td>
            </tr>
            <tr style={{ borderBottom: '1px solid var(--border-color)' }}>
              <td style={{ padding: '0.5rem' }}>
                <strong>Data Rate (Normal)</strong>
              </td>
              <td style={{ padding: '0.5rem' }}>~15.6 bytes/sec</td>
            </tr>
            <tr style={{ borderBottom: '1px solid var(--border-color)' }}>
              <td style={{ padding: '0.5rem' }}>
                <strong>Frequency Band</strong>
              </td>
              <td style={{ padding: '0.5rem' }}>400 Hz - 2300 Hz (sub-bass)</td>
            </tr>
            <tr style={{ borderBottom: '1px solid var(--border-color)' }}>
              <td style={{ padding: '0.5rem' }}>
                <strong>Frequency Bins</strong>
              </td>
              <td style={{ padding: '0.5rem' }}>96 bins with 20 Hz spacing</td>
            </tr>
            <tr style={{ borderBottom: '1px solid var(--border-color)' }}>
              <td style={{ padding: '0.5rem' }}>
                <strong>FEC</strong>
              </td>
              <td style={{ padding: '0.5rem' }}>Reed-Solomon (255, 223) with shortened optimization</td>
            </tr>
            <tr style={{ borderBottom: '1px solid var(--border-color)' }}>
              <td style={{ padding: '0.5rem' }}>
                <strong>Error Correction</strong>
              </td>
              <td style={{ padding: '0.5rem' }}>Up to 16 byte errors per block</td>
            </tr>
            <tr>
              <td style={{ padding: '0.5rem' }}>
                <strong>Max Payload</strong>
              </td>
              <td style={{ padding: '0.5rem' }}>200 bytes per transmission</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div className="card mt-5">
        <h2>Speed Modes</h2>
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <tbody>
            <tr style={{ borderBottom: '1px solid var(--border-color)' }}>
              <td style={{ padding: '0.5rem' }}>
                <strong>Normal</strong>
              </td>
              <td style={{ padding: '0.5rem' }}>192ms/symbol</td>
              <td style={{ padding: '0.5rem' }}>~15.6 bytes/sec</td>
              <td style={{ padding: '0.5rem' }}>Maximum robustness</td>
            </tr>
            <tr style={{ borderBottom: '1px solid var(--border-color)' }}>
              <td style={{ padding: '0.5rem' }}>
                <strong>Fast</strong>
              </td>
              <td style={{ padding: '0.5rem' }}>96ms/symbol</td>
              <td style={{ padding: '0.5rem' }}>~31.2 bytes/sec</td>
              <td style={{ padding: '0.5rem' }}>Balanced speed/reliability</td>
            </tr>
            <tr>
              <td style={{ padding: '0.5rem' }}>
                <strong>Fastest</strong>
              </td>
              <td style={{ padding: '0.5rem' }}>48ms/symbol</td>
              <td style={{ padding: '0.5rem' }}>~62.5 bytes/sec</td>
              <td style={{ padding: '0.5rem' }}>Maximum speed</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div className="card mt-5" style={{ background: '#f0fdf4', borderLeft: '4px solid #22c55e' }}>
        <h3 style={{ color: '#15803d', marginTop: 0 }}>💡 Architecture</h3>
        <p style={{ color: '#15803d', marginBottom: 0 }}>
          FSK-only implementation optimized for over-the-air audio transmission. Multi-tone frequency selection provides
          redundancy and robustness. Sub-bass frequency band (400-2300 Hz) ensures excellent acoustic propagation through
          typical room environments with high speaker and microphone compatibility.
        </p>
      </div>
    </div>
  )
}

export default IndexPage
