"use client"

import { useEffect, useState, useCallback } from "react"
import { useQuery } from "@tanstack/react-query"
import Header from "@/components/Header"
import ChatPanel from "@/components/ChatPanel"
import NetworkStatus from "@/components/NetworkStatus"
import NetworkVisualizer from "@/components/NetworkVisualizer"
import {
    fetchTopology,
    probeLocalShard,
    startScoutWorker,
    type Topology,
} from "@/lib/swarm"
import {
    initWebLLM,
    checkWebGPUSupport,
    type ModelProgress,
} from "@/lib/webllm"

export type NodeMode =
    | "loading"
    | "local-shard"
    | "scout-initializing"
    | "scout"
    | "leech"

export default function HomePage() {
    const [mode, setMode] = useState<NodeMode>("loading")
    const [webLLMProgress, setWebLLMProgress] = useState<ModelProgress | null>(null)
    const [webLLMError, setWebLLMError] = useState<string | null>(null)
    const [pitchMode, setPitchMode] = useState(false)
    const [toastMessage, setToastMessage] = useState<string | null>(null)

    // React Query for topology polling
    const { data: topology } = useQuery({
        queryKey: ["topology"],
        queryFn: fetchTopology,
        refetchInterval: 10000, // Poll every 10 seconds
        staleTime: 5000,
    })

    const rustStatus: "connected" | "unreachable" = topology?.status === "ok" ? "connected" : "unreachable"
    const topologyData: Topology | null = topology ?? null

    useEffect(() => {
        const boot = async () => {
            // 1. Check for local Shard exe
            const probe = await probeLocalShard()
            if (probe.available) {
                setMode("local-shard")
                return
            }

            // If no local shard was found, initialize WebLLM for Scout mode
            if (!probe.available) {
                setMode("scout-initializing")

                try {
                    // Check WebGPU support first
                    const gpuStatus = await checkWebGPUSupport()
                    if (!gpuStatus.supported) {
                        setWebLLMError(
                            `WebGPU not available: ${gpuStatus.reason}. Cannot run Scout mode.`
                        )
                        setMode("leech")
                        return
                    }

                    // Initialize WebLLM with progress callback
                    await initWebLLM((progress) => {
                        setWebLLMProgress(progress)
                    })

                    // Clear progress and transition to scout mode
                    setWebLLMProgress(null)
                    setWebLLMError(null)
                    setMode("scout")

                    // Start the Scout worker loop
                    startScoutWorker(
                        (work) => {
                            console.log("Received work:", work.workId)
                        },
                        (result) => {
                            console.log("Work result:", result)
                        }
                    )
                } catch (error: any) {
                    console.error("Failed to initialize WebLLM:", error)
                    setWebLLMError(error?.message ?? "Failed to initialize Scout mode")
                    setMode("leech")
                }
            }
        }

        boot()
    }, [])

    // Pitch Mode keyboard shortcut (Ctrl+Shift+P)
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.ctrlKey && e.shiftKey && e.key === "P") {
                e.preventDefault()
                setPitchMode(prev => !prev)
            }
        }
        
        window.addEventListener("keydown", handleKeyDown)
        return () => window.removeEventListener("keydown", handleKeyDown)
    }, [])

    // Toast notification handler
    const handleToast = useCallback((message: string) => {
        setToastMessage(message)
        setTimeout(() => setToastMessage(null), 4000)
    }, [])

    return (
        <div className="app-shell">
            <Header mode={mode} rustStatus={rustStatus} />
            
            {/* Toast Notification */}
            {toastMessage && (
                <div
                    style={{
                        position: "fixed",
                        top: "80px",
                        left: "50%",
                        transform: "translateX(-50%)",
                        background: "linear-gradient(135deg, #1e293b, #0f172a)",
                        border: "1px solid rgba(100, 200, 255, 0.3)",
                        borderRadius: "8px",
                        padding: "12px 24px",
                        color: "#fff",
                        fontSize: "13px",
                        fontFamily: "var(--font-mono, monospace)",
                        zIndex: 1000,
                        boxShadow: "0 4px 20px rgba(0, 0, 0, 0.5)",
                        animation: "fadeIn 0.3s ease"
                    }}
                >
                    {toastMessage}
                </div>
            )}
            
            {/* Network Visualizer - shown in pitch mode or when in local-shard mode */}
            {(pitchMode || mode === "local-shard") && (
                <div
                    style={{
                        padding: "16px",
                        borderBottom: "1px solid rgba(100, 200, 255, 0.1)"
                    }}
                >
                    <NetworkVisualizer 
                        pitchMode={pitchMode} 
                        onToast={handleToast}
                    />
                </div>
            )}
            
            <NetworkStatus
                mode={mode}
                topology={topologyData}
                rustStatus={rustStatus}
                webLLMProgress={webLLMProgress}
                webLLMError={webLLMError}
            />
            <ChatPanel mode={mode} />
        </div>
    )
}
