import React from 'react';
import './StateDisplay.css';

const State = ({ icon, title, message }) => (
  <div className="state-display">
    <div className="state-content">
      <div className="state-icon">{icon}</div>
      <div className="state-title">{title}</div>
      <div className="state-message">{message}</div>
    </div>
  </div>
);

export function StateDisplay({ state, message, query }) {
  if (state === 'loading') {
    return <State icon={<div className="spinner"></div>} title="Searching..." />;
  }

  if (state === 'error') {
    return <State icon={<div className="error-icon">!</div>} title="Search Error" message={message} />;
  }

  if (state === 'empty') {
    const icon = (
      <svg width="72" height="72" viewBox="0 0 72 72" fill="none" stroke="currentColor" strokeWidth="1.5">
        <circle cx="32" cy="32" r="18" strokeOpacity="0.5" />
        <path d="M45 45 L60 60" strokeLinecap="round" />
        <circle cx="24" cy="30" r="2" fill="currentColor" />
        <circle cx="32" cy="30" r="2" fill="currentColor" />
        <circle cx="40" cy="30" r="2" fill="currentColor" />
        <path d="M25 38 Q32 44 39 38" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    );
    return <State icon={icon} title={`No results for "${query}"`} message="Try adjusting your keywords or filters." />;
  }

  return null;
}