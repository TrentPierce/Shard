'use client'

import React, { useRef, useEffect } from 'react'

interface PeerNode {
    peerId: string
    role: 'Verifier' | 'Scout'
    latencyMs: number
    tokensPerSecond: number
    isHealthy: boolean
}

interface TopologyEdge {
    source: string
    target: string
    latencyMs: number
    healthy: boolean
}

interface TopologyGraphProps {
    peers: PeerNode[]
    edges: TopologyEdge[]
}

interface SimNode extends PeerNode {
    x: number
    y: number
    vx: number
    vy: number
}

export function TopologyGraph({ peers, edges }: TopologyGraphProps) {
    const canvasRef = useRef<HTMLCanvasElement>(null)
    const nodesRef = useRef<SimNode[]>([])
    const animRef = useRef<number>(0)

    useEffect(() => {
        const canvas = canvasRef.current
        if (!canvas) return

        const ctx = canvas.getContext('2d')
        if (!ctx) return

        const w = canvas.width = canvas.offsetWidth * 2
        const h = canvas.height = canvas.offsetHeight * 2
        ctx.scale(2, 2)
        const dw = w / 2
        const dh = h / 2

        // Initialize or update nodes
        const existing = new Map(nodesRef.current.map(n => [n.peerId, n]))
        const centerX = dw / 2
        const centerY = dh / 2

        const nodes: SimNode[] = peers.map((p, i) => {
            const prev = existing.get(p.peerId)
            if (prev) {
                return { ...p, x: prev.x, y: prev.y, vx: 0, vy: 0 }
            }
            const angle = (2 * Math.PI * i) / Math.max(peers.length, 1)
            const r = Math.min(dw, dh) * 0.3
            return {
                ...p,
                x: centerX + r * Math.cos(angle),
                y: centerY + r * Math.sin(angle),
                vx: 0,
                vy: 0,
            }
        })
        nodesRef.current = nodes

        // Simple force simulation
        function tick() {
            const damping = 0.9
            const repulsion = 2000
            const attraction = 0.005
            const center_gravity = 0.01

            for (const node of nodes) {
                // Center gravity
                node.vx += (centerX - node.x) * center_gravity
                node.vy += (centerY - node.y) * center_gravity

                // Repulsion from other nodes
                for (const other of nodes) {
                    if (other === node) continue
                    const dx = node.x - other.x
                    const dy = node.y - other.y
                    const dist = Math.sqrt(dx * dx + dy * dy) || 1
                    const force = repulsion / (dist * dist)
                    node.vx += (dx / dist) * force
                    node.vy += (dy / dist) * force
                }
            }

            // Attraction along edges
            for (const edge of edges) {
                const src = nodes.find(n => n.peerId === edge.source)
                const tgt = nodes.find(n => n.peerId === edge.target)
                if (!src || !tgt) continue
                const dx = tgt.x - src.x
                const dy = tgt.y - src.y
                src.vx += dx * attraction
                src.vy += dy * attraction
                tgt.vx -= dx * attraction
                tgt.vy -= dy * attraction
            }

            for (const node of nodes) {
                node.vx *= damping
                node.vy *= damping
                node.x += node.vx
                node.y += node.vy
                // Keep in bounds
                node.x = Math.max(20, Math.min(dw - 20, node.x))
                node.y = Math.max(20, Math.min(dh - 20, node.y))
            }
        }

        function draw() {
            tick()
            ctx!.clearRect(0, 0, dw, dh)

            // Draw edges
            for (const edge of edges) {
                const src = nodes.find(n => n.peerId === edge.source)
                const tgt = nodes.find(n => n.peerId === edge.target)
                if (!src || !tgt) continue

                ctx!.beginPath()
                ctx!.moveTo(src.x, src.y)
                ctx!.lineTo(tgt.x, tgt.y)
                ctx!.strokeStyle = edge.healthy
                    ? 'rgba(154, 225, 157, 0.28)'
                    : 'rgba(144, 149, 144, 0.35)'
                ctx!.lineWidth = edge.healthy ? 1 : 2
                ctx!.stroke()

                // Latency label at midpoint
                const mx = (src.x + tgt.x) / 2
                const my = (src.y + tgt.y) / 2
                ctx!.fillStyle = 'rgba(255,255,255,0.3)'
                ctx!.font = '8px Inter, sans-serif'
                ctx!.textAlign = 'center'
                ctx!.fillText(`${edge.latencyMs}ms`, mx, my - 4)
            }

            // Draw nodes
            for (const node of nodes) {
                const radius = node.role === 'Verifier' ? 10 : 7
                const color = node.isHealthy
                    ? (node.role === 'Verifier' ? '#9ae19d' : '#537a5a')
                    : '#909590'

                // Glow
                ctx!.beginPath()
                ctx!.arc(node.x, node.y, radius + 4, 0, 2 * Math.PI)
                ctx!.fillStyle = color.replace(')', ', 0.15)').replace('rgb', 'rgba')
                ctx!.fill()

                // Circle
                ctx!.beginPath()
                ctx!.arc(node.x, node.y, radius, 0, 2 * Math.PI)
                ctx!.fillStyle = color
                ctx!.fill()

                // Label
                ctx!.fillStyle = 'rgba(255,255,255,0.7)'
                ctx!.font = '9px Inter, sans-serif'
                ctx!.textAlign = 'center'
                const label = node.peerId.slice(0, 8) + '...'
                ctx!.fillText(label, node.x, node.y + radius + 14)

                // Role badge
                ctx!.fillStyle = 'rgba(255,255,255,0.4)'
                ctx!.font = '7px Inter, sans-serif'
                ctx!.fillText(node.role, node.x, node.y + radius + 23)
            }

            animRef.current = requestAnimationFrame(draw)
        }

        draw()

        return () => {
            cancelAnimationFrame(animRef.current)
        }
    }, [peers, edges])

    return (
        <div className="topology-container">
            <canvas ref={canvasRef} className="topology-canvas" />
            {peers.length === 0 && (
                <div className="topology-empty">No peers connected</div>
            )}
            <style jsx>{`
        .topology-container {
          position: relative;
          width: 100%;
          height: 250px;
        }
        .topology-canvas {
          width: 100%;
          height: 100%;
        }
        .topology-empty {
          position: absolute;
          inset: 0;
          display: flex;
          align-items: center;
          justify-content: center;
          color: rgba(255,255,255,0.3);
          font-size: 13px;
        }
      `}</style>
        </div>
    )
}
