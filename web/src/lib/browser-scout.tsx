/**
 * Browser Scout Integration
 * 
 * Enables browser-based contribution using WebLLM.
 * Users can generate draft tokens without installing anything.
 */

"use client"

import { useState, useCallback, useEffect } from "react"

interface BrowserScoutConfig {
  model: string
  maxTokens: number
  temperature: number
}

interface ScoutStatus {
  loaded: boolean
  loading: boolean
  progress: number
  error: string | null
}

interface BrowserScoutProps {
  onTokenGenerated?: (token: string) => void
  onStatusChange?: (status: ScoutStatus) => void
}

const DEFAULT_MODEL = "Llama-3.1-1B-Instruct-Q4_K_M"

export function useBrowserScout({ onTokenGenerated, onStatusChange }: BrowserScoutProps = {}) {
  const [status, setStatus] = useState<ScoutStatus>({
    loaded: false,
    loading: false,
    progress: 0,
    error: null
  })
  const [engine, setEngine] = useState<any>(null)

  // Initialize WebLLM engine with PoW
  const initEngine = useCallback(async (config: BrowserScoutConfig = { model: DEFAULT_MODEL, maxTokens: 64, temperature: 0.7 }) => {
    if (typeof window === "undefined" || !("gpu" in navigator)) {
      setStatus(prev => ({ ...prev, error: "WebGPU not supported in this browser" }))
      return
    }

    setStatus(prev => ({ ...prev, loading: true, error: null, progress: 0 }))
    onStatusChange?.({ loading: true, loaded: false, progress: 0, error: null })

    try {
      // 1. Double-Dip / VRAM collision prevention
      const { probeLocalShard } = await import("./swarm")
      const localShard = await probeLocalShard()
      if (localShard.available) {
        throw new Error("Local Shard native node detected. Disabling browser GPU to prevent VRAM crash.")
      }

      // 2. PoW Challenge
      let peerId = localStorage.getItem("shard_scout_id")
      if (!peerId) {
        peerId = `scout_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`
        localStorage.setItem("shard_scout_id", peerId)
      }

      const hardwareConcurrency = navigator.hardwareConcurrency || 4
      const isMobile = /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(navigator.userAgent)

      setStatus(prev => ({ ...prev, progress: 10 })) // Status: Requesting challenge

      const res = await fetch(`/v1/pow/challenge?peer_id=${peerId}&hardware_concurrency=${hardwareConcurrency}&is_mobile=${isMobile}`)
      if (!res.ok) {
        throw new Error(`Failed to fetch PoW challenge: ${res.statusText}`)
      }
      const challengeData = await res.json()

      setStatus(prev => ({ ...prev, progress: 20 })) // Status: Solving PoW

      // Load pow worker
      const workerUrl = new URL('./pow_solver.ts', import.meta.url)
      const worker = new Worker(workerUrl, { type: 'module' })

      const solved = await new Promise<{ nonce: number; hashHex: string }>((resolve, reject) => {
        worker.onmessage = (e) => {
          if (e.data.type === "solved") {
            resolve({ nonce: e.data.nonce, hashHex: e.data.hashHex })
            return
          }
          if (e.data.type === "progress") {
            return
          }
          if (e.data.type === "timeout") {
            reject(new Error("PoW solve timed out"))
            return
          }
          else reject(new Error(e.data.error || "PoW failed"))
          worker.terminate()
        }
        worker.onerror = (e) => {
          reject(e)
          worker.terminate()
        }
        worker.postMessage({
          challengeHex: challengeData.challenge.challenge_bytes_hex,
          difficulty: challengeData.challenge.difficulty,
          hardwareConcurrency,
          isMobile
        })
      })

      setStatus(prev => ({ ...prev, progress: 40 })) // Status: Verifying PoW

      const verifyRes = await fetch(`/v1/pow/verify`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ peer_id: peerId, nonce: solved.nonce, hash_hex: solved.hashHex })
      })
      if (!verifyRes.ok) {
        throw new Error(`Failed to verify PoW: ${verifyRes.statusText}`)
      }
      const verifyData = await verifyRes.json()
      if (!verifyData.ok) {
        throw new Error(`PoW verification rejected`)
      }

      // 3. Load MLCEngine
      setStatus(prev => ({ ...prev, progress: 50 }))

      // Dynamic import for WebLLM
      const webllm = await import("@mlc-ai/web-llm")

      const initProgressCallback = (report: any) => {
        const progress = report.progress || 0
        // Scale 50% to 100%
        setStatus(prev => ({ ...prev, progress: Math.min(100, Math.round(50 + (progress * 50))) }))
      }

      const selectedModel = config.model
      const engine = await webllm.CreateMLCEngine(
        selectedModel,
        {
          initProgressCallback,
        }
      )

      setEngine(engine)
      setStatus({ loaded: true, loading: false, progress: 100, error: null })
      onStatusChange?.({ loaded: true, loading: false, progress: 100, error: null })
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : "Failed to initialize WebLLM"
      setStatus(prev => ({ ...prev, loading: false, error: errorMsg }))
      onStatusChange?.({ loaded: false, loading: false, progress: 0, error: errorMsg })
    }
  }, [onStatusChange])

  // Generate draft tokens
  const generateDraft = useCallback(async (prompt: string): Promise<string[]> => {
    if (!engine) {
      throw new Error("Scout engine not initialized")
    }

    try {
      const output = await engine.chat.completions.create({
        messages: [{ role: "user", content: prompt }],
        max_tokens: 64,
        temperature: 0.7,
      })

      const content = output.choices[0]?.message?.content || ""
      const tokens: string[] = content.split(/\s+/).filter(Boolean)

      tokens.forEach((token: string) => onTokenGenerated?.(token))

      return tokens
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : "Generation failed"
      setStatus(prev => ({ ...prev, error: errorMsg }))
      throw err
    }
  }, [engine, onTokenGenerated])

  // Cleanup
  const destroy = useCallback(async () => {
    if (engine) {
      await engine.terminate()
      setEngine(null)
      setStatus({ loaded: false, loading: false, progress: 0, error: null })
    }
  }, [engine])

  return {
    status,
    initEngine,
    generateDraft,
    destroy,
    isSupported: typeof window !== "undefined" && "gpu" in navigator
  }
}

// Browser Scout Status Display Component
export function BrowserScoutStatus({ status }: { status: ScoutStatus }) {
  if (status.loaded) {
    return (
      <div className="flex items-center gap-2 text-emerald-400">
        <span className="w-2 h-2 bg-emerald-400 rounded-full animate-pulse" />
        <span className="text-sm">Browser Scout Active</span>
      </div>
    )
  }

  if (status.loading) {
    return (
      <div className="flex flex-col gap-2">
        <div className="flex items-center gap-2 text-cyan-400">
          <span className="w-2 h-2 bg-cyan-400 rounded-full animate-pulse" />
          <span className="text-sm">Loading Model... {status.progress}%</span>
        </div>
        <div className="w-32 h-1 bg-slate-700 rounded-full overflow-hidden">
          <div
            className="h-full bg-cyan-500 transition-all"
            style={{ width: `${status.progress}%` }}
          />
        </div>
      </div>
    )
  }

  if (status.error) {
    return (
      <div className="flex items-center gap-2 text-red-400">
        <span className="text-sm">{status.error}</span>
      </div>
    )
  }

  return null
}
