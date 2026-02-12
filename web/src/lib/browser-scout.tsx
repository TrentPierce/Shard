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

  // Initialize WebLLM engine
  const initEngine = useCallback(async (config: BrowserScoutConfig = { model: DEFAULT_MODEL, maxTokens: 64, temperature: 0.7 }) => {
    if (typeof window === "undefined" || !("gpu" in navigator)) {
      setStatus(prev => ({ ...prev, error: "WebGPU not supported in this browser" }))
      return
    }

    setStatus(prev => ({ ...prev, loading: true, error: null, progress: 0 }))
    onStatusChange?.({ loading: true, loaded: false, progress: 0, error: null })

    try {
      // Dynamic import for WebLLM
      const webllm = await import("@mlc-ai/web-llm")
      
      const initProgressCallback = (report: any) => {
        const progress = report.progress || 0
        setStatus(prev => ({ ...prev, progress: Math.round(progress * 100) }))
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
