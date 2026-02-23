"use client"

import { useState, useEffect, useCallback } from "react"
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

// Mock data generators
function generateMockNodes(): NodeInfo[] {
  const regions = ['US-East', 'US-West', 'EU-West', 'EU-Central', 'Asia-Pacific']
  const nodes: NodeInfo[] = []
  
  for (let i = 0; i < 25; i++) {
    const isScout = Math.random() > 0.3
    nodes.push({
      id: `node_${Math.random().toString(36).substring(7)}`,
      type: isScout ? 'Scout' : 'Shard',
      region: regions[Math.floor(Math.random() * regions.length)],
      status: Math.random() > 0.2 ? 'active' : 'idle',
      uptime: `${Math.floor(Math.random() * 24)}h ${Math.floor(Math.random() * 60)}m`,
      draftsToday: isScout ? Math.floor(Math.random() * 500) : 0
    })
  }
  return nodes
}

function generateMockActivity(): ActivityEvent[] {
  const events: ActivityEvent[] = []
  const types: ActivityEvent['type'][] = ['scout_join', 'scout_leave', 'request_complete', 'shard_join', 'shard_leave']
  
  for (let i = 0; i < 15; i++) {
    const type = types[Math.floor(Math.random() * types.length)]
    let message = ''
    
    switch (type) {
      case 'scout_join':
        message = `Scout [abc${Math.floor(Math.random() * 1000)}] joined from Chrome/Windows`
        break
      case 'scout_leave':
        message = `Scout [def${Math.floor(Math.random() * 1000)}] went offline`
        break
      case 'request_complete':
        const tokens = Math.floor(Math.random() * 100) + 20
        const rounds = Math.floor(Math.random() * 5) + 1
        const accepted = Math.floor(Math.random() * 30) + 70
        message = `Request completed — ${tokens} tokens, ${rounds} rounds, accepted ${accepted}%`
        break
      case 'shard_join':
        message = `Shard node [ghi${Math.floor(Math.random() * 1000)}] came online`
        break
      case 'shard_leave':
        message = `Shard node [jkl${Math.floor(Math.random() * 1000)}] went offline`
        break
    }
    
    events.push({
      id: `event_${i}`,
      type,
      message,
      timestamp: Date.now() - (i * 60000)
    })
  }
  return events
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
      // Use Promise.allSettled to handle partial failures gracefully
      const [topologyRes, metricsRes, healthRes, peersRes] = await Promise.allSettled([
        fetch("/api/v1/system/topology"),
        fetch("/api/v1/metrics/summary"),
        fetch("/api/health"),
        fetch("/api/v1/system/peers"),
      ])

      // Helper to extract JSON from fulfilled results
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
          region: "Bootstrap",
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
          region: "Unknown",
          status: peer.verified ? "active" : "idle",
          uptime,
          draftsToday: 0,
        })
      }

      setNodes(nodeList)

      const activityEvents: ActivityEvent[] = nodeList.map((node, index) => ({
        id: `event_${index}`,
        type: "shard_join",
        message: `Shard node [${node.id.substring(0, 12)}] is ${node.status}`,
        timestamp: now,
      }))

      setActivity(activityEvents)
    } catch {
      // leave previous stats/nodes/activity on failure
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchData()
    // Auto-refresh every 10 seconds
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
      title: "Run the Shard daemon",
      command:
        'shard-daemon --control-port 9091 --tcp-port 4001 --webrtc-port 9090 --quic-port 9092 --bootstrap-node "/ip4/35.175.242.222/tcp/4001/p2p/12D3KooWConhJakwyGN72uZ1Jtxi3LFecN3cYKxEX3aNLDAo48by"',
      description: "Start a verifier node connected to the public Shard mesh"
    },
    {
      title: "Open the web interface",
      command: "open https://shardnetwork.live",
      description: "Visit the live network dashboard and chat interface"
    },
    {
      title: "Start contributing",
      command: "# Just keep the tab open",
      description: "Your browser will automatically contribute compute"
    }
  ]

  if (loading) {
    return (
      <div className="min-h-screen bg-slate-950 text-white flex items-center justify-center">
        <div className="text-center">
          <RefreshCw className="w-8 h-8 animate-spin mx-auto mb-4 text-cyan-400" />
          <p className="text-slate-400">Loading network data...</p>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-slate-950 text-white">
      {/* Header */}
      <header className="border-b border-slate-800">
        <div className="max-w-7xl mx-auto px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Globe className="w-6 h-6 text-cyan-400" />
            <span className="font-bold text-lg">Shard</span>
            <span className="text-slate-500">/</span>
            <span className="text-slate-400">Network Explorer</span>
          </div>
          <div className="flex items-center gap-4">
            <span className="text-sm text-slate-500">Auto-refreshes every 10s</span>
            <button 
              onClick={fetchData}
              className="p-2 hover:bg-slate-800 rounded-lg transition-colors"
            >
              <RefreshCw className="w-4 h-4" />
            </button>
          </div>
        </div>
      </header>

      <main className="max-w-7xl mx-auto px-6 py-8">
        {/* NETWORK STATS */}
        <section className="mb-8">
          <h2 className="text-xl font-semibold mb-4 flex items-center gap-2">
            <Activity className="w-5 h-5 text-cyan-400" />
            Network Statistics
          </h2>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div className="bg-slate-900 rounded-lg p-6">
              <div className="flex items-center justify-between mb-2">
                <Users className="w-5 h-5 text-purple-400" />
                <span className="text-xs text-slate-500">Live</span>
              </div>
              <div className="text-3xl font-bold">{stats.scouts}</div>
              <div className="text-sm text-slate-400">Active Scouts</div>
            </div>
            
            <div className="bg-slate-900 rounded-lg p-6">
              <div className="flex items-center justify-between mb-2">
                <Server className="w-5 h-5 text-emerald-400" />
                <span className="text-xs text-slate-500">Live</span>
              </div>
              <div className="text-3xl font-bold">{stats.shards}</div>
              <div className="text-sm text-slate-400">Active Shards</div>
            </div>
            
            <div className="bg-slate-900 rounded-lg p-6">
              <div className="flex items-center justify-between mb-2">
                <Clock className="w-5 h-5 text-cyan-400" />
                <span className="text-xs text-slate-500">Now</span>
              </div>
              <div className="text-3xl font-bold">{stats.activeNodes.toLocaleString()}</div>
              <div className="text-sm text-slate-400">Active Nodes</div>
            </div>
            
            <div className="bg-slate-900 rounded-lg p-6">
              <div className="flex items-center justify-between mb-2">
                <Zap className="w-5 h-5 text-yellow-400" />
                <span className="text-xs text-slate-500">Avg</span>
              </div>
              <div className="text-3xl font-bold text-yellow-400">{stats.avgLatencyMs.toFixed(1)}</div>
              <div className="text-sm text-slate-400">Latency (ms)</div>
            </div>
          </div>
        </section>

        {/* MAIN CONTENT GRID */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* NODE LIST - Takes 2 columns */}
          <section className="lg:col-span-2">
            <div className="bg-slate-900 rounded-lg p-6">
              <div className="flex items-center justify-between mb-4">
                <h2 className="text-lg font-semibold flex items-center gap-2">
                  <Cpu className="w-5 h-5 text-cyan-400" />
                  Node List
                  <span className="text-sm font-normal text-slate-500">({nodes.length} nodes)</span>
                </h2>
                <select 
                  value={sortBy}
                  onChange={(e) => setSortBy(e.target.value as any)}
                  className="bg-slate-800 border border-slate-700 rounded px-3 py-1 text-sm"
                >
                  <option value="uptime">Sort by Uptime</option>
                  <option value="contribution">Sort by Contribution</option>
                  <option value="region">Sort by Region</option>
                </select>
              </div>
              
              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="text-left text-slate-400 text-sm border-b border-slate-800">
                      <th className="pb-3 font-medium">Node ID</th>
                      <th className="pb-3 font-medium">Type</th>
                      <th className="pb-3 font-medium">Region</th>
                      <th className="pb-3 font-medium">Status</th>
                      <th className="pb-3 font-medium">Uptime</th>
                      <th className="pb-3 font-medium">Drafts Today</th>
                    </tr>
                  </thead>
                  <tbody>
                    {sortedNodes.slice(0, 15).map((node) => (
                      <tr key={node.id} className="border-b border-slate-800/50">
                        <td className="py-3 font-mono text-sm">{node.id.substring(0, 12)}...</td>
                        <td className="py-3">
                          <span className={`px-2 py-1 rounded text-xs ${
                            node.type === 'Scout' 
                              ? 'bg-purple-500/20 text-purple-400' 
                              : 'bg-emerald-500/20 text-emerald-400'
                          }`}>
                            {node.type}
                          </span>
                        </td>
                        <td className="py-3 text-slate-400">{node.region}</td>
                        <td className="py-3">
                          <span className={`flex items-center gap-1 ${
                            node.status === 'active' ? 'text-green-400' : 'text-slate-500'
                          }`}>
                            <div className={`w-2 h-2 rounded-full ${
                              node.status === 'active' ? 'bg-green-400' : 'bg-slate-500'
                            }`} />
                            {node.status}
                          </span>
                        </td>
                        <td className="py-3 text-slate-400">{node.uptime}</td>
                        <td className="py-3 text-slate-400">{node.draftsToday}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              
              {nodes.length > 15 && (
                <div className="mt-4 text-center text-sm text-slate-500">
                  Showing 15 of {nodes.length} nodes. Pagination coming soon.
                </div>
              )}
            </div>
          </section>

          {/* LIVE ACTIVITY FEED */}
          <section>
            <div className="bg-slate-900 rounded-lg p-6 h-full">
              <h2 className="text-lg font-semibold mb-4 flex items-center gap-2">
                <Activity className="w-5 h-5 text-cyan-400" />
                Live Activity
                <span className="text-xs font-normal text-slate-500">(last 20)</span>
              </h2>
              
              <div className="space-y-3 max-h-[500px] overflow-y-auto">
                {activity.map((event) => (
                  <div key={event.id} className="text-sm p-3 bg-slate-800/50 rounded">
                    <div className="flex items-start gap-2">
                      <div className={`w-2 h-2 rounded-full mt-1.5 flex-shrink-0 ${
                        event.type.includes('join') ? 'bg-green-400' :
                        event.type.includes('leave') ? 'bg-red-400' :
                        'bg-cyan-400'
                      }`} />
                      <div>
                        <p className="text-slate-300">{event.message}</p>
                        <p className="text-xs text-slate-500 mt-1">
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
        <section className="mt-8">
          <div className="bg-slate-900 rounded-lg p-6">
            <h2 className="text-xl font-semibold mb-6 flex items-center gap-2">
              <Terminal className="w-5 h-5 text-cyan-400" />
              How to Join the Network
            </h2>
            
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              {joinSteps.map((step, index) => (
                <div key={index} className="relative">
                  <div className="bg-slate-800 rounded-lg p-4 h-full">
                    <div className="flex items-center gap-2 mb-3">
                      <div className="w-6 h-6 bg-cyan-500/20 rounded-full flex items-center justify-center text-cyan-400 text-sm font-bold">
                        {index + 1}
                      </div>
                      <h3 className="font-medium">{step.title}</h3>
                    </div>
                    <p className="text-sm text-slate-400 mb-3">{step.description}</p>
                    <div className="relative">
                      <code className="block bg-slate-950 rounded p-2 text-sm font-mono text-cyan-400 overflow-x-auto">
                        {step.command}
                      </code>
                      <button
                        onClick={() => copyToClipboard(step.command, index)}
                        className="absolute top-2 right-2 p-1 hover:bg-slate-700 rounded transition-colors"
                        title="Copy to clipboard"
                      >
                        {copiedStep === index ? (
                          <Check className="w-4 h-4 text-green-400" />
                        ) : (
                          <Copy className="w-4 h-4 text-slate-400" />
                        )}
                      </button>
                    </div>
                  </div>
                  {index < joinSteps.length - 1 && (
                    <ArrowRight className="hidden md:block absolute -right-4 top-1/2 -translate-y-1/2 w-5 h-5 text-slate-600" />
                  )}
                </div>
              ))}
            </div>
            
            <div className="mt-6 pt-6 border-t border-slate-800 flex flex-wrap gap-4">
              <a 
                href="/join"
                className="inline-flex items-center gap-2 px-4 py-2 bg-cyan-500 hover:bg-cyan-400 text-slate-950 rounded-lg font-medium transition-colors"
              >
                <Globe className="w-4 h-4" />
                Join as Scout (Browser)
              </a>
              <a 
                href="https://github.com/TrentPierce/Shard/releases"
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center gap-2 px-4 py-2 bg-slate-800 hover:bg-slate-700 rounded-lg font-medium transition-colors"
              >
                <Server className="w-4 h-4" />
                Download Shard Daemon
                <ExternalLink className="w-3 h-3" />
              </a>
            </div>
          </div>
        </section>
      </main>
    </div>
  )
}
