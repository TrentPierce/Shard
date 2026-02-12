"use client"

import { useEffect, useState } from "react"
import { useQuery } from "@tanstack/react-query"
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
    const [webLLMProgress, setWebLLMProgress] = useState<ModelProgress | null>(null)
    const [webLLMError, setWebLLMError] = useState<string | null>(null)

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
            // 1. Check for local Oracle exe
            const probe = await probeLocalOracle()
            if (probe.available) {
                setMode("local-oracle")
                return
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
    }, [])

    return (
        <div className="app-shell">
            <Header mode={mode} rustStatus={rustStatus} />
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
