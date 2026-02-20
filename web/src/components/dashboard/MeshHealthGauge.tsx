'use client'

import React from 'react'

interface MeshHealthGaugeProps {
    peerCount: number
    avgLatency: number
    sigFailureRate: number
}

export function MeshHealthGauge({ peerCount, avgLatency, sigFailureRate }: MeshHealthGaugeProps) {
    // Calculate health score: 0-100
    const peerScore = Math.min(peerCount / 10, 1) * 40   // 40% weight
    const latencyScore = Math.max(0, 1 - avgLatency / 200) * 35  // 35% weight
    const sigScore = Math.max(0, 1 - sigFailureRate / 10) * 25  // 25% weight
    const healthScore = Math.round(peerScore + latencyScore + sigScore)

    const circumference = 2 * Math.PI * 60
    const offset = circumference - (healthScore / 100) * circumference

    const color =
        healthScore >= 80 ? '#4ade80' :
            healthScore >= 50 ? '#fbbf24' :
                '#f87171'

    return (
        <div className="gauge-container">
            <svg viewBox="0 0 140 140" className="gauge-svg">
                {/* Background circle */}
                <circle
                    cx="70" cy="70" r="60"
                    fill="none"
                    stroke="rgba(255,255,255,0.06)"
                    strokeWidth="10"
                />
                {/* Value arc */}
                <circle
                    cx="70" cy="70" r="60"
                    fill="none"
                    stroke={color}
                    strokeWidth="10"
                    strokeDasharray={circumference}
                    strokeDashoffset={offset}
                    strokeLinecap="round"
                    transform="rotate(-90 70 70)"
                    style={{ transition: 'stroke-dashoffset 0.8s ease, stroke 0.5s ease' }}
                />
                {/* Center text */}
                <text x="70" y="65" textAnchor="middle" fontSize="28" fontWeight="700" fill={color}>
                    {healthScore}
                </text>
                <text x="70" y="85" textAnchor="middle" fontSize="10" fill="rgba(255,255,255,0.5)">
                    HEALTH SCORE
                </text>
            </svg>
            <div className="gauge-details">
                <div className="gauge-detail">
                    <span className="detail-icon">🔗</span>
                    <span className="detail-value">{peerCount}</span>
                    <span className="detail-label">Peers</span>
                </div>
                <div className="gauge-detail">
                    <span className="detail-icon">⏱</span>
                    <span className="detail-value">{avgLatency.toFixed(0)}ms</span>
                    <span className="detail-label">Avg Latency</span>
                </div>
                <div className="gauge-detail">
                    <span className="detail-icon">🛡</span>
                    <span className="detail-value">{sigFailureRate.toFixed(1)}%</span>
                    <span className="detail-label">Sig Failures</span>
                </div>
            </div>

            <style jsx>{`
        .gauge-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 16px;
        }
        .gauge-svg {
          width: 140px;
          height: 140px;
        }
        .gauge-details {
          display: flex;
          gap: 24px;
        }
        .gauge-detail {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 2px;
        }
        .detail-icon { font-size: 16px; }
        .detail-value {
          font-size: 14px;
          font-weight: 600;
          color: rgba(255,255,255,0.9);
          font-variant-numeric: tabular-nums;
        }
        .detail-label {
          font-size: 10px;
          color: rgba(255,255,255,0.4);
          text-transform: uppercase;
        }
      `}</style>
        </div>
    )
}
