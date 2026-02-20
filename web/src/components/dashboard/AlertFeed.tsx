'use client'

import React from 'react'

interface AlertEvent {
    id: string
    kind: 'ddos' | 'latency_spike' | 'sig_failure' | 'sybil' | 'info'
    message: string
    severity: 'critical' | 'warning' | 'info'
    timestamp: number
}

interface AlertFeedProps {
    alerts: AlertEvent[]
}

const SEVERITY_COLORS = {
    critical: '#f87171',
    warning: '#fbbf24',
    info: '#60a5fa',
}

const KIND_ICONS: Record<string, string> = {
    ddos: '🛡',
    latency_spike: '⚡',
    sig_failure: '🔐',
    sybil: '👥',
    info: 'ℹ️',
}

export function AlertFeed({ alerts }: AlertFeedProps) {
    if (alerts.length === 0) {
        return (
            <div className="alert-empty">
                <span className="empty-icon">✅</span>
                <span>No active alerts</span>
                <style jsx>{`
          .alert-empty {
            display: flex; flex-direction: column; align-items: center;
            justify-content: center; height: 200px; gap: 8px;
            color: rgba(255,255,255,0.3); font-size: 13px;
          }
          .empty-icon { font-size: 28px; }
        `}</style>
            </div>
        )
    }

    return (
        <div className="alert-feed">
            {alerts.slice().reverse().map((alert) => (
                <div key={alert.id} className={`alert-item severity-${alert.severity}`}>
                    <span className="alert-icon">{KIND_ICONS[alert.kind] || '⚠️'}</span>
                    <div className="alert-content">
                        <span className="alert-message">{alert.message}</span>
                        <span className="alert-time">
                            {new Date(alert.timestamp).toLocaleTimeString()}
                        </span>
                    </div>
                    <span
                        className="alert-badge"
                        style={{ background: SEVERITY_COLORS[alert.severity] }}
                    >
                        {alert.severity}
                    </span>
                </div>
            ))}

            <style jsx>{`
        .alert-feed {
          display: flex;
          flex-direction: column;
          gap: 8px;
        }
        .alert-item {
          display: flex;
          align-items: center;
          gap: 12px;
          padding: 10px 12px;
          border-radius: 10px;
          background: rgba(255,255,255,0.03);
          border-left: 3px solid transparent;
          transition: background 0.2s;
        }
        .alert-item:hover {
          background: rgba(255,255,255,0.06);
        }
        .alert-item.severity-critical {
          border-left-color: #f87171;
        }
        .alert-item.severity-warning {
          border-left-color: #fbbf24;
        }
        .alert-item.severity-info {
          border-left-color: #60a5fa;
        }
        .alert-icon {
          font-size: 18px;
          flex-shrink: 0;
        }
        .alert-content {
          flex: 1;
          display: flex;
          flex-direction: column;
          gap: 2px;
        }
        .alert-message {
          font-size: 13px;
          color: rgba(255,255,255,0.85);
          line-height: 1.3;
        }
        .alert-time {
          font-size: 10px;
          color: rgba(255,255,255,0.35);
          font-variant-numeric: tabular-nums;
        }
        .alert-badge {
          font-size: 9px;
          font-weight: 600;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          padding: 3px 8px;
          border-radius: 10px;
          color: white;
          flex-shrink: 0;
        }
      `}</style>
        </div>
    )
}
