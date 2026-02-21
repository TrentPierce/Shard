"use client"

import React, { createContext, useContext, useState, useEffect, useRef, useCallback } from "react"
import { useQuery } from "@tanstack/react-query"
import {
    fetchTopology,
    probeLocalShard,
    initP2P,
    startScoutWorker,
    subscribeToWork,
    subscribeToResults,
    publishResult,
    type Topology,
} from "@/lib/swarm"
import { PREFER_LOCAL_SHARD } from "@/lib/config"
import {
    initWebLLM,
    checkWebGPUSupport,
    type ModelProgress,
} from "@/lib/webllm"
import { startBrowserLayerHost } from "@/lib/layer-host"

export type NodeMode =
    | "loading"
    | "local-shard"
    | "scout-initializing"
    | "scout"
    | "leech"

interface AppContextType {
    mode: NodeMode
    topology: Topology | null
    rustStatus: "connected" | "unreachable" | "downloading"
    webLLMProgress: ModelProgress | null
    webLLMError: string | null
    retryScout: () => void
}

const AppContext = createContext<AppContextType | undefined>(undefined)

export function AppProvider({ children }: { children: React.ReactNode }) {
    const [mode, setMode] = useState<NodeMode>("loading")
    const [webLLMProgress, setWebLLMProgress] = useState<ModelProgress | null>(null)
    const [webLLMError, setWebLLMError] = useState<string | null>(null)
    const [scoutRetryNonce, setScoutRetryNonce] = useState(0)
    const scoutBootedRef = useRef(false)
    const stopScoutWorkerRef = useRef<(() => void) | null>(null)
    const stopLayerHostRef = useRef<(() => void) | null>(null)

    const { data: topology, refetch: refetchTopologyData } = useQuery({
        queryKey: ["topology"],
        queryFn: fetchTopology,
        refetchInterval: 10000,
        staleTime: 5000,
    })

    const rustStatus = (topology?.status === "ok" ? "connected" : "unreachable") as "connected" | "unreachable" | "downloading"

    const retryScout = useCallback(() => {
        if (stopScoutWorkerRef.current) {
            stopScoutWorkerRef.current()
            stopScoutWorkerRef.current = null
        }
        if (stopLayerHostRef.current) {
            stopLayerHostRef.current()
            stopLayerHostRef.current = null
        }
        scoutBootedRef.current = false
        setWebLLMError(null)
        setWebLLMProgress(null)
        setScoutRetryNonce((prev) => prev + 1)
    }, [])

    const getBootstrapPeersFromTopology = useCallback((topo: Topology | null): string[] => {
        if (!topo) return []
        const candidates: string[] = []
        const peerId = topo.shard_peer_id ?? ""
        const addWithPeerId = (addr: string | null | undefined) => {
            if (!addr) return
            const trimmed = addr.trim()
            if (!trimmed) return
            if (trimmed.includes("/p2p/")) {
                candidates.push(trimmed)
                return
            }
            if (peerId) {
                candidates.push(`${trimmed}/p2p/${peerId}`)
            } else {
                candidates.push(trimmed)
            }
        }
        addWithPeerId(topo.shard_webrtc_multiaddr)
        addWithPeerId(topo.shard_ws_multiaddr)
        addWithPeerId(topo.shard_quic_multiaddr ?? null)
        for (const listenAddr of topo.listen_addrs ?? []) {
            addWithPeerId(listenAddr)
        }
        const isHttps = typeof window !== "undefined" && window.location.protocol === "https:"
        return candidates.filter((addr) => {
            if (!isHttps) return true
            const insecureWs = addr.startsWith("ws://") || addr.includes("/ws/") || addr.endsWith("/ws")
            if (!insecureWs) return true
            return addr.startsWith("wss://") || addr.includes("/wss/")
        })
    }, [])

    useEffect(() => {
        if (scoutBootedRef.current) return
        const boot = async () => {
            scoutBootedRef.current = true
            
            try {
                // Ensure we have fresh topology data immediately
                await refetchTopologyData()

                if (PREFER_LOCAL_SHARD) {
                    const probe = await probeLocalShard()
                    if (probe.available) {
                        setMode("local-shard")
                        return
                    }
                }
                
                setMode("scout-initializing")
                const gpuStatus = await checkWebGPUSupport()
                
                if (!gpuStatus.supported) {
                    console.warn(`[Shard] WebGPU unavailable (${gpuStatus.reason}), defaulting to Consumer mode`)
                    setWebLLMError(`WebGPU not available: ${gpuStatus.reason}`)
                    setMode("leech")
                    return
                }

                await initWebLLM((progress) => setWebLLMProgress(progress))
                setWebLLMProgress(null)
                setWebLLMError(null)
                setMode("scout")
                
                try {
                    stopScoutWorkerRef.current = await startScoutWorker()
                } catch (e) { console.error(e) }
                
                try {
                    stopLayerHostRef.current = await startBrowserLayerHost({ modelId: "default-model", layerStart: 0, layerEnd: 1 })
                } catch (e) { console.warn(e) }
                
                try {
                    const liveTopology = await fetchTopology()
                    const bootstrapPeers = getBootstrapPeersFromTopology(liveTopology)
                    await initP2P({ emitSelf: false, bootstrapPeers: bootstrapPeers.length > 0 ? bootstrapPeers : undefined })
                    subscribeToWork((work) => console.log("Work:", work.request_id))
                    subscribeToResults((result) => { publishResult(result) })
                } catch (e) { console.error(e) }

            } catch (error: any) {
                console.error("[Shard] Boot failed:", error)
                setWebLLMError(error?.message ?? "Failed to initialize Scout")
                setMode("leech")
            }
        }
        boot()
        return () => {
            stopScoutWorkerRef.current?.()
            stopLayerHostRef.current?.()
        }
    }, [scoutRetryNonce, getBootstrapPeersFromTopology, refetchTopologyData])

    return (
        <AppContext.Provider value={{
            mode,
            topology: topology ?? null,
            rustStatus,
            webLLMProgress,
            webLLMError,
            retryScout
        }}>
            {children}
        </AppContext.Provider>
    )
}

export function useAppContext() {
    const context = useContext(AppContext)
    if (context === undefined) {
        throw new Error("useAppContext must be used within an AppProvider")
    }
    return context
}
