"use client"

import React, { createContext, useContext, useState, useEffect, useRef, useCallback } from "react"
import { useQuery } from "@tanstack/react-query"
import {
    fetchTopology,
    probeLocalShard,
    startScoutWorker,
    type Topology,
} from "@/lib/swarm"
import { PREFER_LOCAL_SHARD, apiUrl } from "@/lib/config"
import {
    initWebLLM,
    type ModelProgress,
} from "@/lib/webllm"
import { startBrowserLayerHost } from "@/lib/layer-host"
import { detectScoutCapability } from "@/lib/scout-capability"
import {
    setContributionStatus,
    type ContributionStatus,
} from "@/lib/contribution-status"

export type NodeMode =
    | "loading"
    | "local-shard"
    | "scout-initializing"
    | "scout"
    | "leech"

type RustStatus = "connected" | "degraded" | "unreachable" | "downloading"

interface AppContextType {
    mode: NodeMode
    topology: Topology | null
    rustStatus: RustStatus
    webLLMProgress: ModelProgress | null
    webLLMError: string | null
    contributionStatus: ContributionStatus | null
    retryScout: () => void
}

const AppContext = createContext<AppContextType | undefined>(undefined)

export function AppProvider({ children }: { children: React.ReactNode }) {
    const [mode, setMode] = useState<NodeMode>("loading")
    const [webLLMProgress, setWebLLMProgress] = useState<ModelProgress | null>(null)
    const [webLLMError, setWebLLMError] = useState<string | null>(null)
    const [contributionStatus, setContributionStatusState] = useState<ContributionStatus | null>(null)
    const [scoutRetryNonce, setScoutRetryNonce] = useState(0)
    const [healthStatus, setHealthStatus] = useState<"ok" | "degraded" | "unreachable">("degraded")
    const scoutBootedRef = useRef(false)
    const stopScoutWorkerRef = useRef<(() => void) | null>(null)
    const stopLayerHostRef = useRef<(() => void) | null>(null)

    const { data: topology, refetch: refetchTopologyData } = useQuery({
        queryKey: ["topology"],
        queryFn: fetchTopology,
        refetchInterval: 10000,
        staleTime: 5000,
    })

    useEffect(() => {
        const pollHealth = async () => {
            try {
                const res = await fetch(apiUrl("/health"), { cache: "no-store" })
                if (!res.ok) {
                    setHealthStatus("unreachable")
                    return
                }
                const payload = await res.json()
                setHealthStatus(payload?.status === "ok" ? "ok" : "degraded")
            } catch {
                setHealthStatus("unreachable")
            }
        }

        pollHealth()
        const interval = setInterval(pollHealth, 10000)
        return () => clearInterval(interval)
    }, [])

    const retryScout = useCallback(() => {
        stopScoutWorkerRef.current?.()
        stopLayerHostRef.current?.()
        stopScoutWorkerRef.current = null
        stopLayerHostRef.current = null
        scoutBootedRef.current = false
        setWebLLMError(null)
        setWebLLMProgress(null)
        setContributionStatusState(
            setContributionStatus("initializing", "Retrying browser scout startup")
        )
        setScoutRetryNonce((prev) => prev + 1)
    }, [])

    const getBootstrapPeersFromTopology = useCallback((topo: Topology | null): string[] => {
        if (!topo) return []
        const candidates: string[] = []
        const peerId = topo.shard_peer_id ?? ""
        const addWithPeerId = (addr: string | null | undefined) => {
            if (!addr?.trim()) return
            if (addr.includes("/p2p/")) {
                candidates.push(addr)
                return
            }
            candidates.push(peerId ? `${addr}/p2p/${peerId}` : addr)
        }

        addWithPeerId(topo.shard_webrtc_multiaddr)
        addWithPeerId(topo.shard_ws_multiaddr)
        addWithPeerId(topo.shard_quic_multiaddr ?? null)
        for (const listenAddr of topo.listen_addrs ?? []) addWithPeerId(listenAddr)

        const isHttps = typeof window !== "undefined" && window.location.protocol === "https:"
        return candidates.filter((addr) => {
            if (!isHttps) return true
            const insecureWs = addr.startsWith("ws://") || addr.includes("/ws/") || addr.endsWith("/ws")
            return !insecureWs || addr.startsWith("wss://") || addr.includes("/wss/")
        })
    }, [])

    useEffect(() => {
        if (scoutBootedRef.current) return

        const boot = async () => {
            scoutBootedRef.current = true

            try {
                await refetchTopologyData()

                if (PREFER_LOCAL_SHARD) {
                    const probe = await probeLocalShard()
                    if (probe.available) {
                        setMode("local-shard")
                        setContributionStatusState(
                            setContributionStatus(
                                "not_contributing",
                                "Local verifier detected; browser scout disabled to avoid GPU contention."
                            )
                        )
                        return
                    }
                }

                setMode("scout-initializing")
                const capability = await detectScoutCapability()
                if (capability.capability !== "webgpu") {
                    const reason =
                        capability.capability === "wasm"
                            ? "WASM fallback detected; browser scout contribution is disabled in production mode."
                            : `WebGPU not available: ${capability.reason ?? "unsupported browser capability"}`
                    setWebLLMError(reason)
                    setMode("leech")
                    setContributionStatusState(
                        setContributionStatus("not_contributing", reason, capability.capability)
                    )
                    return
                }

                await initWebLLM((progress) => setWebLLMProgress(progress))
                setWebLLMProgress(null)
                setWebLLMError(null)
                setMode("scout")
                setContributionStatusState(
                    setContributionStatus("contributing", "WebGPU scout active and contribution loop started", capability.capability)
                )

                try {
                    stopScoutWorkerRef.current = await startScoutWorker()
                } catch (e) {
                    console.error(e)
                    setContributionStatusState(
                        setContributionStatus(
                            "degraded",
                            "WebGPU is ready but scout work loop failed to start. Retry required.",
                            capability.capability
                        )
                    )
                }

                try {
                    stopLayerHostRef.current = await startBrowserLayerHost({ modelId: "default-model", layerStart: 0, layerEnd: 1 })
                } catch (e) {
                    console.warn(e)
                }

                try {
                    const liveTopology = await fetchTopology()
                    const bootstrapPeers = getBootstrapPeersFromTopology(liveTopology)
                    const p2p = await import("@/lib/p2p")
                    await p2p.initP2P({ emitSelf: false, bootstrapPeers: bootstrapPeers.length > 0 ? bootstrapPeers : undefined })
                    p2p.subscribeToWork((work) => console.log("Work:", work.request_id))
                    p2p.subscribeToResults((result) => {
                        p2p.publishResult(result)
                    })
                } catch (e) {
                    console.error(e)
                }
            } catch (error: any) {
                setWebLLMError(error?.message ?? "Failed to initialize Scout")
                setMode("leech")
                setContributionStatusState(
                    setContributionStatus(
                        "degraded",
                        error?.message ?? "Failed to initialize Scout"
                    )
                )
            }
        }

        boot()
        return () => {
            stopScoutWorkerRef.current?.()
            stopLayerHostRef.current?.()
        }
    }, [scoutRetryNonce, getBootstrapPeersFromTopology, refetchTopologyData])

    const rustStatus: RustStatus =
        healthStatus === "ok" && topology?.status === "ok"
            ? "connected"
            : healthStatus === "unreachable"
                ? "unreachable"
                : "degraded"

    return (
        <AppContext.Provider
            value={{
                mode,
                topology: topology ?? null,
                rustStatus,
                webLLMProgress,
                webLLMError,
                contributionStatus,
                retryScout,
            }}
        >
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
