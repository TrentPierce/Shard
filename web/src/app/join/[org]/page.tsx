'use client'

import { useState, useEffect, useCallback } from 'react'
import { useParams } from 'next/navigation'
import { 
  Zap, 
  CheckCircle, 
  AlertCircle, 
  Loader2, 
  Users, 
  ArrowRight,
  Server,
  Globe,
  Cpu
} from 'lucide-react'

interface PilotStatus {
  orgName: string
  scoutCount: number
  maxScouts: number
  serverStatus: 'online' | 'offline' | 'starting'
  webGpuSupported: boolean
}

// Browser capability check
async function checkWebGPU(): Promise<boolean> {
  if (!navigator.gpu) return false
  try {
    const adapter = await navigator.gpu.requestAdapter()
    return !!adapter
  } catch {
    return false
  }
}

export default function JoinPage() {
  const params = useParams()
  const orgName = params.org as string || 'Unknown Organization'
  
  const [status, setStatus] = useState<PilotStatus | null>(null)
  const [loading, setLoading] = useState(true)
  const [checkingGpu, setCheckingGpu] = useState(true)
  const [webGpuSupported, setWebGpuSupported] = useState(false)
  const [joining, setJoining] = useState(false)
  const [joined, setJoined] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Fetch pilot status
  const fetchStatus = useCallback(async () => {
    try {
      const res = await fetch(`/api/pilot/status?org=${encodeURIComponent(orgName)}`)
      if (res.ok) {
        const data = await res.json()
        setStatus(data)
      }
    } catch (err) {
      // Daemon not running - that's okay for the landing view
      console.log('Daemon not available')
    } finally {
      setLoading(false)
    }
  }, [orgName])

  // Initial load
  useEffect(() => {
    // Check WebGPU
    checkWebGPU().then(supported => {
      setWebGpuSupported(supported)
      setCheckingGpu(false)
    })

    // Fetch status
    fetchStatus()
    
    // Poll for status updates
    const interval = setInterval(fetchStatus, 5000)
    return () => clearInterval(interval)
  }, [fetchStatus])

  const handleJoin = async () => {
    setJoining(true)
    setError(null)
    
    try {
      // Register as a Scout
      const res = await fetch('/api/pilot/join', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ org: orgName })
      })
      
      if (res.ok) {
        setJoined(true)
      } else {
        const data = await res.json()
        setError(data.error || 'Failed to join')
      }
    } catch (err) {
      setError('Connection failed. Please try again.')
    } finally {
      setJoining(false)
    }
  }

  if (loading || checkingGpu) {
    return (
      <div className="min-h-screen bg-slate-950 text-white flex items-center justify-center">
        <div className="text-center">
          <Loader2 className="w-8 h-8 animate-spin mx-auto mb-4 text-cyan-400" />
          <p className="text-slate-400">Loading...</p>
        </div>
      </div>
    )
  }

  if (joined) {
    return (
      <div className="min-h-screen bg-slate-950 text-white">
        <div className="max-w-2xl mx-auto px-6 py-20 text-center">
          <div className="w-20 h-20 bg-green-500/20 rounded-full flex items-center justify-center mx-auto mb-6">
            <CheckCircle className="w-10 h-10 text-green-400" />
          </div>
          <h1 className="text-3xl font-bold mb-4">You're Connected!</h1>
          <p className="text-slate-400 mb-8">
            Your browser is now contributing compute to {orgName}'s private AI network.
            You can minimize this tab and continue browsing—the Scout runs in the background.
          </p>
          <div className="bg-slate-900 rounded-lg p-6 text-left">
            <h3 className="font-semibold mb-3 text-slate-300">What happens next:</h3>
            <ul className="space-y-2 text-slate-400">
              <li className="flex items-center gap-2">
                <Zap className="w-4 h-4 text-yellow-400" />
                When someone needs AI, your browser generates draft tokens
              </li>
              <li className="flex items-center gap-2">
                <Cpu className="w-4 h-4 text-cyan-400" />
                The verifier checks your drafts and streams the response
              </li>
              <li className="flex items-center gap-2">
                <Users className="w-4 h-4 text-purple-400" />
                Your contribution helps the whole team get free AI access
              </li>
            </ul>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-slate-950 text-white">
      {/* Header */}
      <header className="border-b border-slate-800">
        <div className="max-w-4xl mx-auto px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Globe className="w-6 h-6 text-cyan-400" />
            <span className="font-bold text-lg">Shard</span>
          </div>
          <div className="text-sm text-slate-400">
            Pilot Network
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="max-w-2xl mx-auto px-6 py-12">
        {/* Org Welcome */}
        <div className="text-center mb-10">
          <div className="inline-flex items-center gap-2 bg-cyan-500/10 text-cyan-400 px-4 py-2 rounded-full text-sm mb-4">
            <Server className="w-4 h-4" />
            {orgName}
          </div>
          <h1 className="text-3xl font-bold mb-3">
            Join Your Team's Private AI Network
          </h1>
          <p className="text-slate-400">
            Contribute your browser's compute to help your team access free AI.
          </p>
        </div>

        {/* Browser Compatibility */}
        <div className="bg-slate-900 rounded-lg p-6 mb-6">
          <h2 className="font-semibold mb-4 flex items-center gap-2">
            <CheckCircle className={`w-5 h-5 ${webGpuSupported ? 'text-green-400' : 'text-yellow-400'}`} />
            Browser Compatibility
          </h2>
          
          {webGpuSupported ? (
            <div className="flex items-center gap-3 text-green-400">
              <CheckCircle className="w-5 h-5" />
              <span>Your browser supports WebGPU. You're ready to go!</span>
            </div>
          ) : (
            <div className="flex items-start gap-3 text-yellow-400">
              <AlertCircle className="w-5 h-5 mt-0.5" />
              <div>
                <p className="font-medium">WebGPU not available</p>
                <p className="text-sm text-slate-400 mt-1">
                  Your browser doesn't support WebGPU. Try Chrome 113+, Edge 113+, or 
                  enable WebGPU in settings. You can still browse the app, but 
                  you won't be able to contribute as a Scout.
                </p>
              </div>
            </div>
          )}
        </div>

        {/* Live Scout Count */}
        {status && (
          <div className="bg-slate-900 rounded-lg p-6 mb-6">
            <h2 className="font-semibold mb-4 flex items-center gap-2">
              <Users className="w-5 h-5 text-purple-400" />
              Current Network Status
            </h2>
            <div className="grid grid-cols-2 gap-4">
              <div className="bg-slate-800 rounded-lg p-4">
                <div className="text-3xl font-bold text-purple-400">{status.scoutCount}</div>
                <div className="text-sm text-slate-400">Active Scouts</div>
              </div>
              <div className="bg-slate-800 rounded-lg p-4">
                <div className="text-3xl font-bold text-cyan-400">{status.maxScouts}</div>
                <div className="text-sm text-slate-400">Maximum Capacity</div>
              </div>
            </div>
          </div>
        )}

        {/* Join Button */}
        <button
          onClick={handleJoin}
          disabled={joining || !webGpuSupported}
          className={`w-full py-4 rounded-lg font-semibold flex items-center justify-center gap-2 transition-all
            ${webGpuSupported 
              ? 'bg-cyan-500 hover:bg-cyan-400 text-slate-950' 
              : 'bg-slate-700 text-slate-400 cursor-not-allowed'
            }`}
        >
          {joining ? (
            <>
              <Loader2 className="w-5 h-5 animate-spin" />
              Connecting...
            </>
          ) : (
            <>
              Start Contributing Compute
              <ArrowRight className="w-5 h-5" />
            </>
          )}
        </button>

        {/* Error Message */}
        {error && (
          <div className="mt-4 p-4 bg-red-500/10 border border-red-500/20 rounded-lg text-red-400 text-sm">
            {error}
          </div>
        )}

        {/* How It Works */}
        <div className="mt-10 pt-10 border-t border-slate-800">
          <h2 className="text-xl font-semibold mb-6 text-center">How It Works</h2>
          <div className="space-y-4">
            <div className="flex gap-4">
              <div className="w-8 h-8 bg-cyan-500/20 rounded-full flex items-center justify-center flex-shrink-0 text-cyan-400 font-bold">
                1
              </div>
              <div>
                <h3 className="font-medium">Your browser runs a lightweight AI model</h3>
                <p className="text-sm text-slate-400">
                  A small model (around 100MB) loads in your browser and generates 
                  draft token suggestions.
                </p>
              </div>
            </div>
            <div className="flex gap-4">
              <div className="w-8 h-8 bg-cyan-500/20 rounded-full flex items-center justify-center flex-shrink-0 text-cyan-400 font-bold">
                2
              </div>
              <div>
                <h3 className="font-medium">Your drafts help the team</h3>
                <p className="text-sm text-slate-400">
                  When someone in your organization uses AI, your browser contributes 
                  draft tokens to speed up the response.
                </p>
              </div>
            </div>
            <div className="flex gap-4">
              <div className="w-8 h-8 bg-cyan-500/20 rounded-full flex items-center justify-center flex-shrink-0 text-cyan-400 font-bold">
                3
              </div>
              <div>
                <h3 className="font-medium">The network verifies and responds</h3>
                <p className="text-sm text-slate-400">
                  The verifier node checks your drafts and streams the final 
                  response back—all in milliseconds.
                </p>
              </div>
            </div>
          </div>
        </div>
      </main>
    </div>
  )
}
