import React from 'react'
import { Link } from 'react-router-dom'

const IndexPage: React.FC = () => {
  const demos = [
    {
      path: '/demo',
      icon: '📝',
      title: 'Main Demo',
      description: 'Encode text to audio and decode audio back to text with real-time WAV file generation.',
    },
    {
      path: '/microphone',
      icon: '🎤',
      title: 'Microphone Detection',
      description: 'Real-time detection of the ascending chirp preamble from your microphone input.',
    },
    {
      path: '/postamble',
      icon: '🎯',
      title: 'Postamble Detection',
      description: 'Real-time detection of the descending chirp postamble from your microphone input.',
    },
    {
      path: '/recording-decode',
      icon: '🎙️',
      title: 'Live Recording & Decode',
      description: 'Record from microphone, automatically detect boundaries, and decode the message.',
    },
    {
      path: '/preamble-postamble-record',
      icon: '🎯',
      title: 'Auto-Record on Preamble',
      description: 'Listen for preamble to auto-start recording, stop on postamble or timeout, then save/decode.',
    },
  ]

  return (
    <div className="container">
      <div className="text-center">
        <h1>transmitwave</h1>
        <p style={{ fontSize: '1.25rem', opacity: 0.9 }}>Audio Modem for Reliable Low-Bandwidth Communication</p>
      </div>

      <div className="grid" style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: '2rem', marginTop: '3rem' }}>
        {demos.map((demo) => (
          <Link key={demo.path} to={demo.path} style={{ textDecoration: 'none', color: 'inherit' }}>
            <div className="card transition-all" style={{ cursor: 'pointer', height: '100%' }}>
              <h3>{demo.icon} {demo.title}</h3>
              <p>{demo.description}</p>
              <button className="btn-primary w-full">Open Demo</button>
            </div>
          </Link>
        ))}
      </div>

      <div className="card mt-5">
        <h2>Features</h2>
        <ul style={{ listStyle: 'none', padding: 0 }}>
          <li style={{ margin: '0.5rem 0' }}>✅ Spread spectrum encoding for noise immunity</li>
          <li style={{ margin: '0.5rem 0' }}>✅ Reed-Solomon error correction (223→255 bytes)</li>
          <li style={{ margin: '0.5rem 0' }}>✅ OFDM with 48 subcarriers (200Hz-4000Hz)</li>
          <li style={{ margin: '0.5rem 0' }}>✅ 16kHz sample rate</li>
          <li style={{ margin: '0.5rem 0' }}>✅ 200 byte max payload</li>
          <li style={{ margin: '0.5rem 0' }}>✅ Real-time preamble/postamble detection</li>
        </ul>
      </div>

      <div className="card mt-5">
        <h2>Technical Specifications</h2>
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <tbody>
            {[
              ['Sample Rate', '16,000 Hz'],
              ['Symbol Duration', '100 ms'],
              ['OFDM Subcarriers', '48'],
              ['Frequency Band', '200 Hz - 4000 Hz'],
              ['Preamble Duration', '250 ms'],
              ['Postamble Duration', '250 ms'],
              ['FEC', 'Reed-Solomon (223, 255)'],
              ['Max Payload', '200 bytes'],
            ].map(([label, value]) => (
              <tr key={label} style={{ borderBottom: '1px solid var(--border-color)' }}>
                <td style={{ padding: '0.5rem' }}><strong>{label}</strong></td>
                <td style={{ padding: '0.5rem' }}>{value}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

export default IndexPage
