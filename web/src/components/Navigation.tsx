import React from 'react'
import { Link } from 'react-router-dom'
import './Navigation.css'

const Navigation: React.FC = () => {
  return (
    <nav className="navbar">
      <div className="navbar-container">
        <Link to="/" className="navbar-brand">
          🔊 transmitwave
        </Link>
        <ul className="navbar-menu">
          <li><Link to="/">Home</Link></li>
          <li><Link to="/standard-fsk">Standard FSK</Link></li>
          <li><Link to="/standard-fsk-listen">Standard FSK Listen</Link></li>
          <li><Link to="/signal-detection">Signal Detection</Link></li>
          <li><Link to="/fountain-encode">Fountain Encode</Link></li>
          <li><Link to="/fountain-listen">Fountain Listen</Link></li>
        </ul>
      </div>
    </nav>
  )
}

export default Navigation
