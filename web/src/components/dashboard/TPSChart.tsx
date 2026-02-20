'use client'

import React from 'react'

interface TPSSample {
    timestamp: number
    value: number
}

interface TPSChartProps {
    data: TPSSample[]
}

export function TPSChart({ data }: TPSChartProps) {
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

    const maxVal = Math.max(...data.map(d => d.value), 1)
    const minTime = data[0].timestamp
    const maxTime = data[data.length - 1].timestamp
    const timeRange = maxTime - minTime || 1

    const points = data.map(d => ({
        x: padding.left + ((d.timestamp - minTime) / timeRange) * chartW,
        y: padding.top + chartH - (d.value / maxVal) * chartH,
    }))

    const line = points.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x} ${p.y}`).join(' ')
    const area = `${line} L ${points[points.length - 1].x} ${padding.top + chartH} L ${points[0].x} ${padding.top + chartH} Z`

    const currentVal = data[data.length - 1].value

    return (
        <div className="chart-container">
            <div className="chart-current">
                <span className="current-value">{currentVal.toLocaleString()}</span>
                <span className="current-label">tokens</span>
            </div>
            <svg viewBox={`0 0 ${width} ${height}`} className="chart-svg">
                {/* Grid lines */}
                {[0, 0.25, 0.5, 0.75, 1].map(frac => {
                    const y = padding.top + chartH * (1 - frac)
                    return (
                        <g key={frac}>
                            <line x1={padding.left} y1={y} x2={width - padding.right} y2={y}
                                stroke="rgba(255,255,255,0.06)" strokeDasharray="4,4" />
                            <text x={padding.left - 5} y={y + 4} textAnchor="end"
                                fontSize="9" fill="rgba(255,255,255,0.3)">
                                {Math.round(maxVal * frac)}
                            </text>
                        </g>
                    )
                })}

                {/* Area fill */}
                <path d={area} fill="url(#tpsGradient)" opacity="0.3" />

                {/* Line */}
                <path d={line} fill="none" stroke="#7c83ff" strokeWidth="2"
                    strokeLinejoin="round" strokeLinecap="round" />

                {/* Latest point */}
                <circle cx={points[points.length - 1].x} cy={points[points.length - 1].y}
                    r="3" fill="#7c83ff" />

                <defs>
                    <linearGradient id="tpsGradient" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor="#7c83ff" stopOpacity="0.4" />
                        <stop offset="100%" stopColor="#7c83ff" stopOpacity="0" />
                    </linearGradient>
                </defs>
            </svg>

            <style jsx>{`
        .chart-container { position: relative; }
        .chart-svg { width: 100%; height: 200px; }
        .chart-current {
          position: absolute; top: 8px; right: 12px;
          text-align: right;
        }
        .current-value {
          font-size: 20px; font-weight: 700; color: #7c83ff;
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
