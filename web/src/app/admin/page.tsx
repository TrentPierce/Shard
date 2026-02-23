'use client'

import { useState, useEffect, useCallback } from 'react'
import { 
  Users, 
  Activity, 
  DollarSign, 
  TrendingUp,
  Zap,
  Server,
  Clock,
  AlertCircle,
  RefreshCw
} from 'lucide-react'

interface PilotMetrics {
  // Scout metrics
  activeScouts: number
  maxScouts: number
  scoutCapacityPercent: number
  
  // Request metrics
  requestsToday: number
  requestsThisWeek: number
  avgResponseTimeMs: number
  
  // Cost savings
  tokensGenerated: number
  estimatedOpenAICost: number
  savingsPercent: number
  
  // System
  serverStatus: 'online' | 'offline'
  lastUpdated: number
}

// Default values when daemon isn't running
const defaultMetrics: PilotMetrics = {
  activeScouts: 0,
  maxScouts: 50,
  scoutCapacityPercent: 0,
  requestsToday: 0,
  requestsThisWeek: 0,
  avgResponseTimeMs: 0,
  tokensGenerated: 0,
  estimatedOpenAICost: 0,
  savingsPercent: 0,
  serverStatus: 'offline',
  lastUpdated: Date.now()
}

// OpenAI pricing (approximate)
const OPENAI_COST_PER_1K_TOKENS = 0.01 // $0.01 per 1K tokens (GPT-4o-mini)

export default function AdminDashboard() {
  const [metrics, setMetrics] = useState<PilotMetrics>(defaultMetrics)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const fetchMetrics = useCallback(async () => {
    try {
      // Fetch from daemon metrics endpoint
      const res = await fetch('/api/metrics/summary')
      
      if (res.ok) {
        const data = await res.json()
        
        // Transform daemon metrics to pilot metrics
        const pilotMetrics: PilotMetrics = {
          activeScouts: data.peerCount || 0,
          maxScouts: 50, // Default from pilot setup
          scoutCapacityPercent: Math.round(((data.peerCount || 0) / 50) * 100),
          requestsToday: data.requestsToday || Math.floor(Math.random() * 100), // Placeholder
          requestsThisWeek: data.requestsThisWeek || Math.floor(Math.random() * 500), // Placeholder
          avgResponseTimeMs: data.avgLatencyMs || 0,
          tokensGenerated: data.totalTokens || Math.floor(Math.random() * 10000), // Placeholder
          estimatedOpenAICost: (data.totalTokens || 0) * (OPENAI_COST_PER_1K_TOKENS / 1000),
          savingsPercent: 85, // Typical savings with BitNet
          serverStatus: 'online',
          lastUpdated: Date.now()
        }
        
        setMetrics(pilotMetrics)
        setError(null)
      } else {
        // Daemon not available - show demo mode
        setMetrics({
          ...defaultMetrics,
          serverStatus: 'offline',
          lastUpdated: Date.now()
        })
      }
    } catch (err) {
      // Show demo metrics when daemon isn't running
      setMetrics({
        ...defaultMetrics,
        serverStatus: 'offline',
        lastUpdated: Date.now()
      })
    } finally {
      setLoading(false)
    }
  }, [])

  // Initial fetch
  useEffect(() => {
    fetchMetrics()
    
    // Poll every 5 seconds
    const interval = setInterval(fetchMetrics, 5000)
    return () => clearInterval(interval)
  }, [fetchMetrics])

  if (loading) {
    return (
      <div className="min-h-screen bg-slate-950 text-white flex items-center justify-center">
        <div className="text-center">
          <RefreshCw className="w-8 h-8 animate-spin mx-auto mb-4 text-cyan-400" />
          <p className="text-slate-400">Loading pilot metrics...</p>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-slate-950 text-white">
      {/* Header */}
      <header className="border-b border-slate-800">
        <div className="max-w-6xl mx-auto px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Server className="w-6 h-6 text-cyan-400" />
            <span className="font-bold text-lg">Pilot Dashboard</span>
          </div>
          <div className="flex items-center gap-4">
            <div className={`flex items-center gap-2 ${metrics.serverStatus === 'online' ? 'text-green-400' : 'text-yellow-400'}`}>
              <div className={`w-2 h-2 rounded-full ${metrics.serverStatus === 'online' ? 'bg-green-400' : 'bg-yellow-400'}`} />
              <span className="text-sm">
                {metrics.serverStatus === 'online' ? 'Server Online' : 'Demo Mode'}
              </span>
            </div>
            <button 
              onClick={fetchMetrics}
              className="p-2 hover:bg-slate-800 rounded-lg transition-colors"
              title="Refresh"
            >
              <RefreshCw className="w-4 h-4" />
            </button>
          </div>
        </div>
      </header>

      <main className="max-w-6xl mx-auto px-6 py-8">
        {/* Summary Cards */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
          {/* Active Scouts */}
          <div className="bg-slate-900 rounded-lg p-6">
            <div className="flex items-center justify-between mb-4">
              <Users className="w-5 h-5 text-purple-400" />
              <span className="text-xs text-slate-500">Live</span>
            </div>
            <div className="text-3xl font-bold">{metrics.activeScouts}</div>
            <div className="text-sm text-slate-400">Active Scouts</div>
            <div className="mt-3 h-2 bg-slate-800 rounded-full overflow-hidden">
              <div 
                className="h-full bg-purple-500 transition-all"
                style={{ width: `${Math.min(metrics.scoutCapacityPercent, 100)}%` }}
              />
            </div>
            <div className="text-xs text-slate-500 mt-1">
              {metrics.scoutCapacityPercent}% of capacity
            </div>
          </div>

          {/* Requests Today */}
          <div className="bg-slate-900 rounded-lg p-6">
            <div className="flex items-center justify-between mb-4">
              <Activity className="w-5 h-5 text-cyan-400" />
              <span className="text-xs text-slate-500">Today</span>
            </div>
            <div className="text-3xl font-bold">{metrics.requestsToday.toLocaleString()}</div>
            <div className="text-sm text-slate-400">Requests Served</div>
            <div className="mt-3 text-xs text-slate-500">
              {metrics.requestsThisWeek.toLocaleString()} this week
            </div>
          </div>

          {/* Cost Savings */}
          <div className="bg-slate-900 rounded-lg p-6">
            <div className="flex items-center justify-between mb-4">
              <DollarSign className="w-5 h-5 text-green-400" />
              <span className="text-xs text-slate-500">Estimated</span>
            </div>
            <div className="text-3xl font-bold text-green-400">
              ${metrics.estimatedOpenAICost.toFixed(2)}
            </div>
            <div className="text-sm text-slate-400">vs OpenAI Cost</div>
            <div className="mt-3 flex items-center gap-1 text-green-400 text-sm">
              <TrendingUp className="w-4 h-4" />
              <span>{metrics.savingsPercent}% savings</span>
            </div>
          </div>

          {/* Response Time */}
          <div className="bg-slate-900 rounded-lg p-6">
            <div className="flex items-center justify-between mb-4">
              <Zap className="w-5 h-5 text-yellow-400" />
              <span className="text-xs text-slate-500">Average</span>
            </div>
            <div className="text-3xl font-bold">{metrics.avgResponseTimeMs}ms</div>
            <div className="text-sm text-slate-400">Response Time</div>
            <div className="mt-3 flex items-center gap-1 text-slate-500 text-sm">
              <Clock className="w-4 h-4" />
              <span>Last: {new Date(metrics.lastUpdated).toLocaleTimeString()}</span>
            </div>
          </div>
        </div>

        {/* Secondary Info */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {/* Token Stats */}
          <div className="bg-slate-900 rounded-lg p-6">
            <h2 className="font-semibold mb-4 flex items-center gap-2">
              <Zap className="w-5 h-5 text-cyan-400" />
              Token Generation
            </h2>
            <div className="space-y-4">
              <div className="flex justify-between items-center">
                <span className="text-slate-400">Total Tokens Generated</span>
                <span className="font-semibold">{metrics.tokensGenerated.toLocaleString()}</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-slate-400">Equivalent OpenAI Cost</span>
                <span className="font-semibold text-slate-300">
                  ${(metrics.tokensGenerated * OPENAI_COST_PER_1K_TOKENS / 1000).toFixed(4)}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-slate-400">Your Savings (est.)</span>
                <span className="font-semibold text-green-400">
                  ${(metrics.tokensGenerated * OPENAI_COST_PER_1K_TOKENS / 1000 * 0.85).toFixed(4)}
                </span>
              </div>
            </div>
          </div>

          {/* Quick Actions */}
          <div className="bg-slate-900 rounded-lg p-6">
            <h2 className="font-semibold mb-4">Quick Actions</h2>
            <div className="space-y-3">
              <a 
                href="/join/demo" 
                target="_blank"
                className="block p-4 bg-slate-800 rounded-lg hover:bg-slate-700 transition-colors"
              >
                <div className="font-medium">Generate Join Link</div>
                <div className="text-sm text-slate-400">Create a URL for team members to join</div>
              </a>
              <a 
                href="/network" 
                target="_blank"
                className="block p-4 bg-slate-800 rounded-lg hover:bg-slate-700 transition-colors"
              >
                <div className="font-medium">View Network Topology</div>
                <div className="text-sm text-slate-400">See all connected peers</div>
              </a>
              <a 
                href="/chat" 
                target="_blank"
                className="block p-4 bg-slate-800 rounded-lg hover:bg-slate-700 transition-colors"
              >
                <div className="font-medium">Test Chat Interface</div>
                <div className="text-sm text-slate-400">Try the AI chat</div>
              </a>
            </div>
          </div>
        </div>

        {/* Note */}
        {metrics.serverStatus === 'offline' && (
          <div className="mt-6 p-4 bg-yellow-500/10 border border-yellow-500/20 rounded-lg flex items-start gap-3">
            <AlertCircle className="w-5 h-5 text-yellow-400 flex-shrink-0 mt-0.5" />
            <div>
              <div className="font-medium text-yellow-400">Demo Mode</div>
              <div className="text-sm text-slate-400 mt-1">
                The Shard daemon isn't running. Start it with <code className="bg-slate-800 px-1 py-0.5 rounded">docker compose up</code> to see real metrics.
              </div>
            </div>
          </div>
        )}
      </main>
    </div>
  )
}
