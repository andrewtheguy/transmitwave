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
          <li><Link to="/demo">Demo</Link></li>
          <li><Link to="/ample">Preamble & Postamble</Link></li>
          <li><Link to="/recording-decode">Recording</Link></li>
        </ul>
      </div>
    </nav>
  )
}

export default Navigation
