import React from 'react'

interface StatusProps {
  message: string | null
  type: 'success' | 'error' | 'info' | 'warning'
}

const Status: React.FC<StatusProps> = ({ message, type }) => {
  if (!message) return null

  return (
    <div className={`status status-${type}`} role="alert">
      {message}
    </div>
  )
}

export default Status
