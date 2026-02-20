"use client"

import { useEffect, useState, useCallback, useRef } from "react"
import { useQuery } from "@tanstack/react-query"
import Header from "@/components/Header"
import ChatPanel from "@/components/ChatPanel"
import NetworkStatus from "@/components/NetworkStatus"
import NetworkVisualizer from "@/components/NetworkVisualizer"
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

export default function HomePage() {
    const [mode, setMode] = useState<NodeMode>("loading")
    const [webLLMProgress, setWebLLMProgress] = useState<ModelProgress | null>(null)
    const [webLLMError, setWebLLMError] = useState<string | null>(null)
    const [pitchMode, setPitchMode] = useState(false)
    const [toastMessage, setToastMessage] = useState<string | null>(null)
    const [scoutRetryNonce, setScoutRetryNonce] = useState(0)
    const scoutBootedRef = useRef(false)
    const stopScoutWorkerRef = useRef<(() => void) | null>(null)
    const stopLayerHostRef = useRef<(() => void) | null>(null)

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
        const filtered = candidates.filter((addr) => {
            if (!isHttps) return true
            const insecureWs =
                addr.startsWith("ws://") ||
                addr.includes("/ws/") ||
                addr.endsWith("/ws")
            if (!insecureWs) return true
            return addr.startsWith("wss://") || addr.includes("/wss/")
        })

        return Array.from(new Set(filtered))
    }, [])

    // Check if the user has already entered the app
    useEffect(() => {
        const entered = typeof window !== "undefined" && localStorage.getItem("shard-entered")
        if (entered === "true") {
            setShowLanding(false)
        }
    }, [])

    useEffect(() => {
        const handleRetryScoutInit = () => {
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
        }
        window.addEventListener("shard:retry-scout-init", handleRetryScoutInit)
        return () => {
            window.removeEventListener("shard:retry-scout-init", handleRetryScoutInit)
        }
    }, [])

    // React Query for topology polling
    const { data: topology } = useQuery({
        queryKey: ["topology"],
        queryFn: fetchTopology,
        refetchInterval: 10000,
        staleTime: 5000,
    })

    const rustStatus = (topology?.status === "ok" ? "connected" : "unreachable") as "connected" | "unreachable" | "downloading"
    const topologyData: Topology | null = topology ?? null

        if (scoutBootedRef.current) return

        const boot = async () => {
            scoutBootedRef.current = true
            if (stopScoutWorkerRef.current) {
                stopScoutWorkerRef.current()
                stopScoutWorkerRef.current = null
            }
            if (PREFER_LOCAL_SHARD) {
                const probe = await probeLocalShard()
                if (probe.available) {
                    setMode("local-shard")
                    return
                }
            }

            setMode("scout-initializing")

            try {
                const gpuStatus = await checkWebGPUSupport()
                if (!gpuStatus.supported) {
                    setWebLLMError(
                        `WebGPU not available: ${gpuStatus.reason}. Cannot run Scout mode.`
                    )
                    setMode("leech")
                    return
                }

                await initWebLLM((progress) => {
                    setWebLLMProgress(progress)
                })

                setWebLLMProgress(null)
                setWebLLMError(null)
                setMode("scout")

                // Start Scout API worker loop so browser tabs actually contribute drafts.
                try {
                    const stopWorker = await startScoutWorker()
                    stopScoutWorkerRef.current = stopWorker
                } catch (workerError) {
                    console.error("[scout] Failed to start worker loop:", workerError)
                }

                try {
                    const stopLayerHost = await startBrowserLayerHost({
                        modelId: "default-model",
                        layerStart: 0,
                        layerEnd: 1,
                    })
                    stopLayerHostRef.current = stopLayerHost
                } catch (layerHostError) {
                    console.warn("[layer-host] Browser layer host disabled:", layerHostError)
                }

                try {
                    const liveTopology = await fetchTopology()
                    const bootstrapPeers = getBootstrapPeersFromTopology(liveTopology)
                    const peerId = await initP2P({
                        emitSelf: false,
                        bootstrapPeers: bootstrapPeers.length > 0 ? bootstrapPeers : undefined,
                    })
                    console.log('[p2p] Connected as scout, peerId:', peerId)

                    subscribeToWork((work) => {
                        console.log("Received work:", work.request_id)
                    })
                    subscribeToResults((result) => {
                        console.log("Work result:", result.request_id)
                        publishResult(result)
                    })
                } catch (p2pError) {
                    console.error('[p2p] Failed to initialize P2P:', p2pError)
                }
            } catch (error: any) {
                console.error("Failed to initialize WebLLM:", error)
                setWebLLMError(error?.message ?? "Failed to initialize Scout mode")
                setMode("leech")
                scoutBootedRef.current = false
            }
        }

        boot()
        return () => {
            if (stopScoutWorkerRef.current) {
                stopScoutWorkerRef.current()
                stopScoutWorkerRef.current = null
            }
            if (stopLayerHostRef.current) {
                stopLayerHostRef.current()
                stopLayerHostRef.current = null
            }
        }
    }, [scoutRetryNonce])

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
            {toastMessage && <div className="app-toast">{toastMessage}</div>}

            {/* Network Visualizer - shown in pitch mode or when in local-shard mode */}
            {(pitchMode || mode === "local-shard") && (
                <div className="app-visualizer-wrap">
                    <NetworkVisualizer pitchMode={pitchMode} onToast={handleToast} />
                </div>
            )}

            <div className="app-main">
                <NetworkStatus
                    mode={mode}
                    topology={topologyData}
                    rustStatus={rustStatus}
                    webLLMProgress={webLLMProgress}
                    webLLMError={webLLMError}
                />
                <ChatPanel mode={mode} />
            </div>
        </div>
    )
}
