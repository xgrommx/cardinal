import React, { useState, useCallback } from 'react';

const TABS = [
  { key: 'files', label: 'Files' },
  { key: 'events', label: 'Events' },
];

const StatusBar = ({
  scannedFiles,
  processedEvents,
  isReady,
  searchDurationMs,
  resultCount,
  onTabChange,
}) => {
  const [activeTab, setActiveTab] = useState('files');

  const handleSelect = useCallback(
    (tabKey) => {
      if (tabKey === activeTab) return;
      setActiveTab(tabKey);
      if (typeof onTabChange === 'function') {
        onTabChange(tabKey);
      }
    },
    [activeTab, onTabChange],
  );

  const resultsText =
    typeof resultCount === 'number'
      ? `${resultCount.toLocaleString()} result${resultCount === 1 ? '' : 's'}`
      : '—';
  const durationText = searchDurationMs != null ? `${Math.round(searchDurationMs)}ms` : null;
  const searchDisplay = durationText ? `${resultsText} • ${durationText}` : resultsText;

  return (
    <div className="status-bar">
      <div className="status-left">
        <div className="status-section">
          <span className={`readiness-indicator ${isReady ? 'ready' : 'not-ready'}`}>
            {isReady ? '●' : '○'}
          </span>
          <span className="status-text">{isReady ? 'Ready' : 'Initializing'}</span>
        </div>
        <div className="status-tabs" role="tablist" aria-label="Search status view">
          {TABS.map(({ key, label }) => {
            const isActive = activeTab === key;
            const value =
              key === 'files' ? scannedFiles.toLocaleString() : processedEvents.toLocaleString();
            return (
              <button
                key={key}
                type="button"
                role="tab"
                aria-selected={isActive}
                className={`status-tab ${isActive ? 'is-active' : ''}`}
                data-tone={key}
                onClick={() => handleSelect(key)}
              >
                <span className="status-tab__label">{label}</span>
                <span className="status-tab__value">{value}</span>
              </button>
            );
          })}
        </div>
      </div>

      <div className="status-right">
        <div className="status-section">
          <span className="status-label">Search:</span>
          <span className="status-value" title="Results • Duration">
            {searchDisplay}
          </span>
        </div>
      </div>
    </div>
  );
};

export default StatusBar;
