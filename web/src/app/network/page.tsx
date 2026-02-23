"use client"

import { useState, useEffect, useCallback } from "react"
import Header from "@/components/Header"
import Footer from "@/components/Footer"
import { 
  Users, 
  Server, 
  Activity, 
  Clock, 
  Zap, 
  ArrowRight,
  Copy,
  Check,
  Globe,
  Terminal,
  Cpu,
  RefreshCw,
  ExternalLink
} from "lucide-react"
import { apiUrl } from "@/lib/config"

interface NetworkStats {
  scouts: number
  shards: number
  activeNodes: number
  avgLatencyMs: number
}

interface NodeInfo {
  id: string
  type: 'Scout' | 'Shard'
  region: string
  status: 'active' | 'idle'
  uptime: string
  draftsToday: number
}

interface ActivityEvent {
  id: string
  type: 'scout_join' | 'scout_leave' | 'request_complete' | 'shard_join' | 'shard_leave'
  message: string
  timestamp: number
}

export default function NetworkExplorerPage() {
  const [stats, setStats] = useState<NetworkStats>({
    scouts: 0,
    shards: 0,
    activeNodes: 0,
    avgLatencyMs: 0,
  })
  const [nodes, setNodes] = useState<NodeInfo[]>([])
  const [activity, setActivity] = useState<ActivityEvent[]>([])
  const [loading, setLoading] = useState(true)
  const [sortBy, setSortBy] = useState<'uptime' | 'contribution' | 'region'>('uptime')
  const [copiedStep, setCopiedStep] = useState<number | null>(null)

  const fetchData = useCallback(async () => {
    try {
      const [topologyRes, metricsRes, healthRes, peersRes] = await Promise.allSettled([
        fetch(apiUrl("/v1/system/topology")),
        fetch(apiUrl("/v1/metrics/summary")),
        fetch(apiUrl("/health")),
        fetch(apiUrl("/v1/system/peers")),
      ])

      const getJson = async (result: PromiseSettledResult<Response>) => {
        if (result.status === 'fulfilled' && result.value.ok) {
          try {
            return await result.value.json()
          } catch {
            return null
          }
        }
        return null
      }

      const [topology, metrics, health, peersPayload] = await Promise.all([
        getJson(topologyRes),
        getJson(metricsRes),
        getJson(healthRes),
        getJson(peersRes),
      ])

      const peersList: any[] = Array.isArray(peersPayload?.peers) ? peersPayload.peers : []
      const connectedPeers = peersList.length || Number(peersPayload?.count ?? 0) || 0

      const scouts = Number(health?.active_scouts ?? 0) || 0
      const shardOnline = health?.status === "ok" || topology?.status === "ok"
      const shardsBase = Number(connectedPeers) || 0
      const shards = shardOnline ? Math.max(1, shardsBase) : shardsBase
      const activeNodes = shards + scouts
      const avgLatencyMs = Number(metrics?.average_latency_ms ?? health?.latency_ms ?? 0) || 0

      setStats({
        scouts,
        shards,
        activeNodes,
        avgLatencyMs,
      })

      const now = Date.now()
      const nodeList: NodeInfo[] = []

      if (shardOnline && topology?.shard_peer_id) {
        nodeList.push({
          id: topology.shard_peer_id,
          type: "Shard",
          region: "Local / Bootstrap",
          status: "active",
          uptime: "—",
          draftsToday: 0,
        })
      }

      for (const peer of peersList) {
        const id = String(peer.peer_id || peer.id || "unknown")
        const connectedAt = typeof peer.connected_at === "number" ? peer.connected_at : null

        let uptime = "—"
        if (connectedAt) {
          const diffSec = Math.max(0, Math.floor((now - connectedAt) / 1000))
          const hours = Math.floor(diffSec / 3600)
          const minutes = Math.floor((diffSec % 3600) / 60)
          uptime = `${hours}h ${minutes}m`
        }

        nodeList.push({
          id,
          type: "Shard",
          region: "Mesh",
          status: peer.verified ? "active" : "idle",
          uptime,
          draftsToday: 0,
        })
      }

      setNodes(nodeList)

      const activityEvents: ActivityEvent[] = nodeList.map((node, index) => ({
        id: `event_${index}`,
        type: "shard_join",
        message: `Node [${node.id.substring(0, 12)}] status: ${node.status}`,
        timestamp: now,
      }))

      setActivity(activityEvents)
    } catch {
      // ignore
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchData()
    const interval = setInterval(fetchData, 10000)
    return () => clearInterval(interval)
  }, [fetchData])

  const sortedNodes = [...nodes].sort((a, b) => {
    switch (sortBy) {
      case 'uptime':
        return b.uptime.localeCompare(a.uptime)
      case 'contribution':
        return b.draftsToday - a.draftsToday
      case 'region':
        return a.region.localeCompare(b.region)
      default:
        return 0
    }
  })

  const copyToClipboard = (text: string, step: number) => {
    navigator.clipboard.writeText(text)
    setCopiedStep(step)
    setTimeout(() => setCopiedStep(null), 2000)
  }

  const joinSteps = [
    {
      title: "Run the Shard Desktop App",
      command: "https://github.com/TrentPierce/Shard/releases",
      description: "Download the v0.6.1 installer for Windows or macOS to join the mesh instantly."
    },
    {
      title: "Open the Dashboard",
      command: "open https://shardnetwork.live",
      description: "Visit the network dash to monitor your node's PoC contributions."
    },
    {
      title: "Start Contributing",
      command: "System Tray > Run in Background",
      description: "Shard runs silently, providing verify services to the mesh when idle."
    }
  ]

  if (loading) {
    return (
      <div className="min-h-screen bg-primary flex items-center justify-center">
        <div className="text-center">
          <RefreshCw className="w-8 h-8 animate-spin mx-auto mb-4 text-primary" />
          <p className="text-muted">Loading real-time network topology...</p>
        </div>
      </div>
    )
  }

  return (
    <div className="bg-grid">
      <Header />
      
      <main className="container section">
        {/* Page Title */}
        <section className="text-center mb-4xl">
          <span className="hero-eyebrow">Real-Time Swarm Topology</span>
          <h1>Network Explorer</h1>
          <p className="max-w-2xl mx-auto mt-md">
            Monitor the decentralized distribution of compute across the global Shard mesh. 
            All connections are secured via Ed25519 authenticated noise pipes.
          </p>
        </section>

        {/* NETWORK STATS */}
        <section className="mb-2xl">
          <div className="stats-grid">
            <div className="stat-card">
              <div className="flex items-center justify-between mb-sm">
                <Users size={20} className="text-primary" />
                <span className="badge badge-primary">LIVE</span>
              </div>
              <div className="stat-value">{stats.scouts}</div>
              <div className="stat-label">Active Scouts</div>
            </div>
            
            <div className="stat-card">
              <div className="flex items-center justify-between mb-sm">
                <Server size={20} className="text-secondary" />
                <span className="badge badge-secondary">LIVE</span>
              </div>
              <div className="stat-value">{stats.shards}</div>
              <div className="stat-label">Active Shards</div>
            </div>
            
            <div className="stat-card">
              <div className="flex items-center justify-between mb-sm">
                <Clock size={20} className="text-accent-cyan" />
                <span className="badge" style={{ background: 'rgba(6, 182, 212, 0.1)', color: 'var(--accent-cyan)' }}>NOW</span>
              </div>
              <div className="stat-value">{stats.activeNodes.toLocaleString()}</div>
              <div className="stat-label">Mesh Total</div>
            </div>
            
            <div className="stat-card">
              <div className="flex items-center justify-between mb-sm">
                <Zap size={20} className="text-accent-amber" />
                <span className="badge" style={{ background: 'rgba(245, 158, 11, 0.1)', color: 'var(--accent-amber)' }}>AVG</span>
              </div>
              <div className="stat-value" style={{ color: 'var(--accent-amber)' }}>{stats.avgLatencyMs.toFixed(1)}</div>
              <div className="stat-label">Mesh Latency (ms)</div>
            </div>
          </div>
        </section>

        {/* MAIN CONTENT GRID */}
        <div className="grid grid-3">
          {/* NODE LIST - Takes 2 columns */}
          <section style={{ gridColumn: 'span 2' }}>
            <div className="card h-full">
              <div className="flex items-center justify-between mb-xl">
                <h2 className="flex items-center gap-md" style={{ fontSize: '1.5rem' }}>
                  <Cpu size={24} className="text-primary" />
                  Swarm Inventory
                  <span className="text-muted font-normal" style={{ fontSize: '0.875rem' }}>({nodes.length} active)</span>
                </h2>
                <select 
                  value={sortBy}
                  onChange={(e) => setSortBy(e.target.value as any)}
                  className="btn btn-secondary btn-sm"
                  style={{ width: 'auto' }}
                >
                  <option value="uptime">Sort by Uptime</option>
                  <option value="contribution">Sort by Contribution</option>
                  <option value="region">Sort by Tier</option>
                </select>
              </div>
              
              <div style={{ overflowX: 'auto' }}>
                <table className="comparison-table" style={{ marginTop: 0 }}>
                  <thead>
                    <tr>
                      <th>Node ID</th>
                      <th>Type</th>
                      <th>Class</th>
                      <th>Status</th>
                      <th>Uptime</th>
                    </tr>
                  </thead>
                  <tbody>
                    {sortedNodes.slice(0, 15).map((node) => (
                      <tr key={node.id}>
                        <td className="text-mono" style={{ fontSize: '0.8125rem' }}>{node.id.substring(0, 16)}...</td>
                        <td>
                          <span className={`badge ${node.type === 'Scout' ? 'badge-secondary' : 'badge-primary'}`}>
                            {node.type}
                          </span>
                        </td>
                        <td>{node.region}</td>
                        <td>
                          <span className="flex items-center gap-sm">
                            <div className={`w-2 h-2 rounded-full ${node.status === 'active' ? 'bg-success animate-pulse' : 'bg-muted'}`} />
                            <span style={{ color: node.status === 'active' ? 'var(--success)' : 'var(--text-muted)' }}>
                              {node.status}
                            </span>
                          </span>
                        </td>
                        <td className="text-muted">{node.uptime}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              
              {nodes.length > 15 && (
                <div className="mt-xl text-center">
                  <p className="text-muted" style={{ fontSize: '0.875rem' }}>Showing top 15 nodes. Decentralized sorting applied.</p>
                </div>
              )}
            </div>
          </section>

          {/* LIVE ACTIVITY FEED */}
          <section>
            <div className="card h-full bg-elevated">
              <h2 className="flex items-center gap-md mb-xl" style={{ fontSize: '1.25rem' }}>
                <Activity size={20} className="text-primary" />
                Live Swarm Log
              </h2>
              
              <div className="flex flex-col gap-md" style={{ maxHeight: '600px', overflowY: 'auto' }}>
                {activity.map((event) => (
                  <div key={event.id} className="card card-glass" style={{ padding: 'var(--space-md)' }}>
                    <div className="flex items-start gap-md">
                      <div className={`w-2 h-2 rounded-full mt-1.5 flex-shrink-0 ${
                        event.type.includes('join') ? 'bg-success' :
                        event.type.includes('leave') ? 'bg-error' :
                        'bg-secondary'
                      }`} />
                      <div>
                        <p style={{ fontSize: '0.875rem', color: 'var(--text-primary)' }}>{event.message}</p>
                        <p style={{ fontSize: '0.75rem', color: 'var(--text-muted)', marginTop: '4px' }}>
                          <Clock size={10} style={{ display: 'inline', marginRight: '4px' }} />
                          {Math.floor((Date.now() - event.timestamp) / 60000)}m ago
                        </p>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </section>
        </div>

        {/* HOW TO JOIN */}
        <section className="mt-2xl">
          <div className="card" style={{ background: 'var(--bg-tertiary)', borderColor: 'var(--primary-glow)' }}>
            <h2 className="flex items-center gap-md mb-xl">
              <Terminal size={24} className="text-primary" />
              Scale the Grid: Join the Mesh
            </h2>
            
            <div className="grid grid-3">
              {joinSteps.map((step, index) => (
                <div key={index} className="relative">
                  <div className="card card-glass h-full" style={{ padding: 'var(--space-lg)' }}>
                    <div className="flex items-center gap-sm mb-md">
                      <div className="badge badge-primary" style={{ width: '24px', height: '24px', padding: 0, justifyContent: 'center' }}>
                        {index + 1}
                      </div>
                      <h3 style={{ fontSize: '1rem', marginBottom: 0 }}>{step.title}</h3>
                    </div>
                    <p style={{ fontSize: '0.875rem', marginBottom: 'var(--space-md)' }}>{step.description}</p>
                    <div className="relative">
                      <code className="code-block" style={{ display: 'block', padding: 'var(--space-sm)', fontSize: '0.75rem' }}>
                        {step.command}
                      </code>
                      <button
                        onClick={() => copyToClipboard(step.command, index)}
                        className="btn btn-ghost btn-sm"
                        style={{ position: 'absolute', top: '4px', right: '4px' }}
                      >
                        {copiedStep === index ? <Check size={14} /> : <Copy size={14} />}
                      </button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
            
            <div className="mt-xl pt-xl border-t border-glass flex flex-wrap gap-md">
              <a href="/join" className="btn btn-primary">
                <Globe size={18} />
                Join as Browser Scout
              </a>
              <a 
                href="https://github.com/TrentPierce/Shard/releases"
                target="_blank"
                rel="noopener noreferrer"
                className="btn btn-secondary"
              >
                <Server size={18} />
                Download Verifier Node
                <ExternalLink size={14} />
              </a>
            </div>
          </div>
        </section>
      </main>

      <Footer />

      <style jsx>{`
        .bg-muted { background: var(--text-muted); }
        .border-glass { border-color: var(--border); }
      `}</style>
    </div>
  )
}
