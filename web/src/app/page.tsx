"use client"

import { useEffect, useState } from "react"
import Header from "@/components/Header"
import ChatPanel from "@/components/ChatPanel"
import NetworkStatus from "@/components/NetworkStatus"
import {
    fetchTopology,
    probeLocalOracle,
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
    | "local-oracle"
    | "scout-initializing"
    | "scout"
    | "leech"

export default function HomePage() {
    const [mode, setMode] = useState<NodeMode>("loading")
    const [topology, setTopology] = useState<Topology | null>(null)
    const [rustStatus, setRustStatus] = useState<"connected" | "unreachable">("unreachable")
    const [webLLMProgress, setWebLLMProgress] = useState<ModelProgress | null>(null)
    const [webLLMError, setWebLLMError] = useState<string | null>(null)

    useEffect(() => {
        const boot = async () => {
            // 1. Check for local Oracle exe
            const probe = await probeLocalOracle()
            if (probe.available) {
                setMode("local-oracle")
                return
            }

            // 2. Fetch topology from Python API
            const topo = await fetchTopology()
            setTopology(topo)

            if (topo.status === "ok") {
                setRustStatus("connected")
            }

            // If no local oracle was found, initialize WebLLM for Scout mode
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

        // Poll topology every 10s
        const interval = setInterval(async () => {
            const topo = await fetchTopology()
            setTopology(topo)
            setRustStatus(topo.status === "ok" ? "connected" : "unreachable")
        }, 10000)

        return () => clearInterval(interval)
    }, [])

    return (
        <div className="app-shell">
            <Header mode={mode} rustStatus={rustStatus} />
            <NetworkStatus
                mode={mode}
                topology={topology}
                rustStatus={rustStatus}
                webLLMProgress={webLLMProgress}
                webLLMError={webLLMError}
            />
            <ChatPanel mode={mode} />
        </div>
    )
}
