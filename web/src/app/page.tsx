"use client"

import { useEffect, useState } from "react"
import Header from "@/components/Header"
import ChatPanel from "@/components/ChatPanel"
import NetworkStatus from "@/components/NetworkStatus"
import { fetchTopology, probeLocalOracle, type Topology } from "@/lib/swarm"

export type NodeMode = "loading" | "local-oracle" | "scout" | "leech"

export default function HomePage() {
    const [mode, setMode] = useState<NodeMode>("loading")
    const [topology, setTopology] = useState<Topology | null>(null)
    const [rustStatus, setRustStatus] = useState<"connected" | "unreachable">("unreachable")

    useEffect(() => {
        const boot = async () => {
            // 1. Check for local Oracle exe
            const probe = await probeLocalOracle()
            if (probe.available) {
                setMode("local-oracle")
            }

            // 2. Fetch topology from Python API
            const topo = await fetchTopology()
            setTopology(topo)

            if (topo.status === "ok") {
                setRustStatus("connected")
            }

            // If no local oracle was found, enter scout mode
            if (!probe.available) {
                setMode("scout")
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
            />
            <ChatPanel mode={mode} />
        </div>
    )
}
