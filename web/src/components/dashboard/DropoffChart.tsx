'use client'

import React from 'react'

interface DropoffSample {
    timestamp: number
    count: number
}

interface DropoffChartProps {
    data: DropoffSample[]
}

export function DropoffChart({ data }: DropoffChartProps) {
    if (data.length < 2) {
        return (
            <div className="chart-empty">
                <span>Collecting data...</span>
                <style jsx>{`
          .chart-empty {
            display: flex; align-items: center; justify-content: center;
            height: 200px; color: rgba(255,255,255,0.3); font-size: 13px;
          }
        `}</style>
            </div>
        )
    }

    const width = 500
    const height = 160
    const padding = { top: 10, right: 10, bottom: 25, left: 45 }
    const chartW = width - padding.left - padding.right
    const chartH = height - padding.top - padding.bottom

    // Convert cumulative counts to deltas
    const deltas = data.map((d, i) => ({
        timestamp: d.timestamp,
        delta: i === 0 ? 0 : d.count - data[i - 1].count,
    }))

    const maxDelta = Math.max(...deltas.map(d => d.delta), 1)
    const minTime = deltas[0].timestamp
    const maxTime = deltas[deltas.length - 1].timestamp
    const timeRange = maxTime - minTime || 1

    const barWidth = Math.max(2, chartW / deltas.length - 2)

    return (
        <div className="chart-container">
            <div className="chart-current">
                <span className="current-value">{data[data.length - 1].count}</span>
                <span className="current-label">total dropoffs</span>
            </div>
            <svg viewBox={`0 0 ${width} ${height}`} className="chart-svg">
                {/* Grid lines */}
                {[0, 0.5, 1].map(frac => {
                    const y = padding.top + chartH * (1 - frac)
                    return (
                        <g key={frac}>
                            <line x1={padding.left} y1={y} x2={width - padding.right} y2={y}
                                stroke="rgba(255,255,255,0.06)" strokeDasharray="4,4" />
                            <text x={padding.left - 5} y={y + 4} textAnchor="end"
                                fontSize="9" fill="rgba(255,255,255,0.3)">
                                {Math.round(maxDelta * frac)}
                            </text>
                        </g>
                    )
                })}

                {/* Bars */}
                {deltas.map((d, i) => {
                    const x = padding.left + ((d.timestamp - minTime) / timeRange) * chartW
                    const barH = (d.delta / maxDelta) * chartH
                    const y = padding.top + chartH - barH
                    const color = d.delta > maxDelta * 0.7 ? '#f87171' :
                        d.delta > maxDelta * 0.3 ? '#fbbf24' : '#7c83ff'
                    return (
                        <rect key={i}
                            x={x - barWidth / 2} y={y}
                            width={barWidth} height={Math.max(barH, 1)}
                            fill={color} rx={1} opacity={0.8}
                        />
                    )
                })}
            </svg>

            <style jsx>{`
        .chart-container { position: relative; }
        .chart-svg { width: 100%; height: 200px; }
        .chart-current {
          position: absolute; top: 8px; right: 12px;
          text-align: right;
        }
        .current-value {
          font-size: 20px; font-weight: 700; color: #fbbf24;
          font-variant-numeric: tabular-nums;
        }
        .current-label {
          font-size: 10px; color: rgba(255,255,255,0.4);
          margin-left: 4px; text-transform: uppercase;
        }
      `}</style>
        </div>
    )
}
