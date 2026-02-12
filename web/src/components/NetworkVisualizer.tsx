"use client"

import { useCallback, useEffect, useRef, useState } from "react"
import { useRouter } from "next/navigation"

interface PeerNode {
  id: string
  name: string
  type: "oracle" | "scout" | "local"
  status: "connected" | "disconnected" | "joining"
}

interface Link {
  source: string
  target: string
  strength: number
}

interface GraphData {
  nodes: PeerNode[]
  links: Link[]
}

interface NetworkVisualizerProps {
  pitchMode?: boolean
  onToast?: (message: string) => void
}

export default function NetworkVisualizer({ pitchMode = false, onToast }: NetworkVisualizerProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const animationRef = useRef<number>(0)
  const [dimensions, setDimensions] = useState({ width: 400, height: 300 })
  const [graphData, setGraphData] = useState<GraphData>({
    nodes: [
      { id: "local", name: "Local Oracle", type: "local", status: "connected" }
    ],
    links: []
  })
  const [tps, setTps] = useState(0)
  const [latency, setLatency] = useState(0)
  const [nodePositions, setNodePositions] = useState<Map<string, { x: number; y: number; vx: number; vy: number }>>(new Map())
  const router = useRouter()

  // Initialize node positions
  useEffect(() => {
    const initialPositions = new Map<string, { x: number; y: number; vx: number; vy: number }>()
    const centerX = dimensions.width / 2
    const centerY = dimensions.height / 2
    
    // Local node at center
    initialPositions.set("local", { x: centerX, y: centerY, vx: 0, vy: 0 })
    
    // Random positions for other nodes
    graphData.nodes.forEach(node => {
      if (!initialPositions.has(node.id)) {
        initialPositions.set(node.id, {
          x: Math.random() * dimensions.width,
          y: Math.random() * dimensions.height,
          vx: (Math.random() - 0.5) * 2,
          vy: (Math.random() - 0.5) * 2
        })
      }
    })
    
    setNodePositions(initialPositions)
  }, [graphData.nodes.length, dimensions])

  // Handle resize
  useEffect(() => {
    const handleResize = () => {
      if (containerRef.current) {
        const rect = containerRef.current.getBoundingClientRect()
        setDimensions({ width: rect.width, height: Math.max(250, rect.height) })
      }
    }
    
    handleResize()
    window.addEventListener("resize", handleResize)
    return () => window.removeEventListener("resize", handleResize)
  }, [])

  // Poll for peer data
  useEffect(() => {
    const fetchPeers = async () => {
      try {
        const res = await fetch("/v1/system/peers")
        if (res.ok) {
          const data = await res.json()
          const peers = data.peers || []
          
          setGraphData(prev => {
            const existingIds = new Set(prev.nodes.map(n => n.id))
            const newNodes = peers.map((p: { peer_id: string }) => p.peer_id)
              .filter((id: string) => !existingIds.has(id))
              .map((id: string) => ({
                id,
                name: `Scout ${id.slice(0, 8)}`,
                type: "scout" as const,
                status: "connected" as const
              }))
            
            if (newNodes.length > 0) {
              onToast?.(`Node ${newNodes[0].id.slice(0, 12)}… joined`)
            }
            
            return {
              nodes: [
                { id: "local", name: "Local Oracle", type: "local", status: "connected" },
                ...prev.nodes.filter((n: PeerNode) => n.id !== "local"),
                ...newNodes
              ],
              links: [
                ...prev.links,
                ...newNodes.map((n: PeerNode) => ({ source: "local", target: n.id, strength: 0.5 }))
              ]
            }
          })
        }
      } catch {
        // Daemon unreachable
      }
    }

    // Fetch stats as well
    const fetchStats = async () => {
      try {
        const res = await fetch("/health")
        if (res.ok) {
          const data = await res.json()
          setTps(data.capacity || 0)
          setLatency(data.latency_ms || 0)
        }
      } catch {
        // Ignore
      }
    }

    fetchPeers()
    fetchStats()
    
    const interval = setInterval(() => {
      fetchPeers()
      fetchStats()
    }, 3000)
    
    return () => clearInterval(interval)
  }, [onToast])

  // Force-directed graph simulation
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    const ctx = canvas.getContext("2d")
    if (!ctx) return

    const simulate = () => {
      if (!containerRef.current) return
      
      const positions = new Map(nodePositions)
      const nodes = graphData.nodes
      const links = graphData.links
      const width = dimensions.width
      const height = dimensions.height

      // Apply forces
      nodes.forEach(node => {
        const pos = positions.get(node.id) || { x: width / 2, y: height / 2, vx: 0, vy: 0 }
        
        // Center gravity
        pos.vx += (width / 2 - pos.x) * 0.001
        pos.vy += (height / 2 - pos.y) * 0.001
        
        // Repulsion between nodes
        nodes.forEach(other => {
          if (other.id === node.id) return
          const otherPos = positions.get(other.id) || { x: width / 2, y: height / 2 }
          const dx = pos.x - otherPos.x
          const dy = pos.y - otherPos.y
          const dist = Math.sqrt(dx * dx + dy * dy) || 1
          const force = 500 / (dist * dist)
          pos.vx += (dx / dist) * force
          pos.vy += (dy / dist) * force
        })
        
        // Link attraction
        links.forEach(link => {
          const sourceId = typeof link.source === "string" ? link.source : link.source.id
          const targetId = typeof link.target === "string" ? link.target : link.target.id
          
          if (sourceId === node.id || targetId === node.id) {
            const otherId = sourceId === node.id ? targetId : sourceId
            const otherPos = positions.get(otherId) || { x: width / 2, y: height / 2 }
            const dx = otherPos.x - pos.x
            const dy = otherPos.y - pos.y
            const dist = Math.sqrt(dx * dx + dy * dy) || 1
            const force = (dist - 80) * 0.01
            pos.vx += (dx / dist) * force
            pos.vy += (dy / dist) * force
          }
        })
        
        // Damping
        pos.vx *= 0.9
        pos.vy *= 0.9
        
        // Update position
        pos.x = Math.max(30, Math.min(width - 30, pos.x + pos.vx))
        pos.y = Math.max(30, Math.min(height - 30, pos.y + pos.vy))
        
        positions.set(node.id, pos)
      })

      setNodePositions(positions)

      // Clear and draw
      ctx.clearRect(0, 0, width, height)
      
      // Draw links
      ctx.strokeStyle = "rgba(100, 200, 255, 0.3)"
      ctx.lineWidth = 2
      links.forEach(link => {
        const sourceId = typeof link.source === "string" ? link.source : link.source.id
        const targetId = typeof link.target === "string" ? link.target : link.target.id
        const sourcePos = positions.get(sourceId)
        const targetPos = positions.get(targetId)
        
        if (sourcePos && targetPos) {
          ctx.beginPath()
          ctx.moveTo(sourcePos.x, sourcePos.y)
          ctx.lineTo(targetPos.x, targetPos.y)
          ctx.stroke()
        }
      })

      // Draw nodes
      nodes.forEach(node => {
        const pos = positions.get(node.id)
        if (!pos) return

        // Node circle
        ctx.beginPath()
        ctx.arc(pos.x, pos.y, node.type === "local" ? 20 : 14, 0, Math.PI * 2)
        
        if (node.type === "local") {
          ctx.fillStyle = "#10b981"
        } else if (node.status === "joining") {
          ctx.fillStyle = "#f59e0b"
        } else {
          ctx.fillStyle = "#3b82f6"
        }
        ctx.fill()
        
        // Node border
        ctx.strokeStyle = node.type === "local" ? "#059669" : "#1d4ed8"
        ctx.lineWidth = 2
        ctx.stroke()

        // Node label
        ctx.fillStyle = "#ffffff"
        ctx.font = "10px system-ui"
        ctx.textAlign = "center"
        ctx.fillText(node.name.slice(0, 12), pos.x, pos.y + 30)
        
        // Status indicator
        if (node.status === "joining") {
          ctx.fillStyle = "#f59e0b"
          ctx.font = "9px system-ui"
          ctx.fillText("joining...", pos.x, pos.y + 42)
        }
      })

      animationRef.current = requestAnimationFrame(simulate)
    }

    simulate()

    return () => {
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current)
      }
    }
  }, [graphData, nodePositions, dimensions])

  // Pitch Mode: Spawn Bot
  const spawnBot = useCallback(() => {
    const botId = `bot-${Date.now()}`
    const newNode: PeerNode = {
      id: botId,
      name: `Scout ${botId.slice(4, 12)}`,
      type: "scout",
      status: "connected"
    }
    
    setGraphData(prev => ({
      nodes: [...prev.nodes, newNode],
      links: [...prev.links, { source: "local", target: botId, strength: 0.8 }]
    }))
    
    setTps(prev => prev + Math.floor(Math.random() * 20) + 10)
    onToast?.(`✨ Spawned Scout Node: ${botId.slice(0, 12)}…`)
  }, [onToast])

  // Pitch Mode: Kill Bot
  const killBot = useCallback(() => {
    setGraphData(prev => {
      const scouts = prev.nodes.filter(n => n.type === "scout" && n.id.startsWith("bot-"))
      if (scouts.length === 0) return prev
      
      const toKill = scouts[Math.floor(Math.random() * scouts.length)]
      const killedId = toKill.id
      
      setTps(prev => Math.max(0, prev - Math.floor(Math.random() * 15) - 5))
      onToast?.(`⚠ Node ${killedId.slice(0, 12)}… Lost. Rerouting chunk #4092... Success.`)
      
      return {
        nodes: prev.nodes.filter(n => n.id !== killedId),
        links: prev.links.filter(l => {
          const sid = typeof l.source === "string" ? l.source : l.source.id
          const tid = typeof l.target === "string" ? l.target : l.target.id
          return sid !== killedId && tid !== killedId
        })
      }
    })
  }, [onToast])

  // Keyboard shortcut for pitch mode
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === "P") {
        e.preventDefault()
        if (!pitchMode) {
          // Toggle pitch mode
          window.dispatchEvent(new CustomEvent("toggle-pitch-mode"))
        }
      }
    }
    
    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [pitchMode])

  return (
    <div 
      ref={containerRef}
      className="network-visualizer"
      style={{
        width: "100%",
        height: "100%",
        minHeight: "250px",
        background: "linear-gradient(135deg, rgba(15, 23, 42, 0.9) 0%, rgba(30, 41, 59, 0.9) 100%)",
        borderRadius: "12px",
        border: "1px solid rgba(100, 200, 255, 0.2)",
        position: "relative",
        overflow: "hidden"
      }}
    >
      <canvas
        ref={canvasRef}
        width={dimensions.width}
        height={dimensions.height}
        style={{
          width: "100%",
          height: "100%",
          display: "block"
        }}
      />
      
      {/* Stats overlay */}
      <div
        style={{
          position: "absolute",
          top: "12px",
          left: "12px",
          background: "rgba(0, 0, 0, 0.6)",
          padding: "8px 12px",
          borderRadius: "6px",
          fontSize: "11px",
          fontFamily: "var(--font-mono, monospace)",
          color: "rgba(255, 255, 255, 0.9)"
        }}
      >
        <div style={{ color: "#10b981", fontWeight: "bold" }}>
          TPS: {tps}
        </div>
        <div style={{ color: "#60a5fa", marginTop: "4px" }}>
          Latency: {latency.toFixed(1)}ms
        </div>
        <div style={{ color: "#a78bfa", marginTop: "4px" }}>
          Nodes: {graphData.nodes.length}
        </div>
      </div>

      {/* Pitch Mode Controls */}
      {pitchMode && (
        <div
          style={{
            position: "absolute",
            bottom: "12px",
            left: "50%",
            transform: "translateX(-50%)",
            display: "flex",
            gap: "8px"
          }}
        >
          <button
            onClick={spawnBot}
            style={{
              background: "linear-gradient(135deg, #10b981, #059669)",
              border: "none",
              borderRadius: "6px",
              padding: "8px 16px",
              color: "white",
              fontSize: "12px",
              fontWeight: "bold",
              cursor: "pointer",
              boxShadow: "0 2px 8px rgba(16, 185, 129, 0.4)"
            }}
          >
            ✨ Spawn Bot
          </button>
          <button
            onClick={killBot}
            style={{
              background: "linear-gradient(135deg, #ef4444, #dc2626)",
              border: "none",
              borderRadius: "6px",
              padding: "8px 16px",
              color: "white",
              fontSize: "12px",
              fontWeight: "bold",
              cursor: "pointer",
              boxShadow: "0 2px 8px rgba(239, 68, 68, 0.4)"
            }}
          >
            💀 Kill Bot
          </button>
        </div>
      )}

      {/* Pitch Mode indicator */}
      <div
        style={{
          position: "absolute",
          top: "12px",
          right: "12px",
          background: pitchMode ? "rgba(245, 158, 11, 0.9)" : "rgba(100, 200, 255, 0.2)",
          padding: "4px 10px",
          borderRadius: "4px",
          fontSize: "10px",
          fontWeight: "bold",
          color: pitchMode ? "white" : "rgba(255, 255, 255, 0.7)",
          textTransform: "uppercase",
          letterSpacing: "0.5px"
        }}
      >
        {pitchMode ? "🎤 PITCH MODE" : "Network Graph"}
      </div>
    </div>
  )
}

// Helper hook to use in parent components
export function usePitchMode() {
  const [enabled, setEnabled] = useState(false)
  const [toast, setToast] = useState<string | null>(null)

  useEffect(() => {
    const handleToggle = () => setEnabled(prev => !prev)
    window.addEventListener("toggle-pitch-mode", handleToggle)
    return () => window.removeEventListener("toggle-pitch-mode", handleToggle)
  }, [])

  const showToast = useCallback((message: string) => {
    setToast(message)
    setTimeout(() => setToast(null), 4000)
  }, [])

  return { enabled, toast, showToast }
}
