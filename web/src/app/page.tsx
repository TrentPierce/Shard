"use client"

import { useEffect, useState, useCallback } from "react"
import Header from "@/components/Header"
import ChatPanel from "@/components/ChatPanel"
import NetworkStatus from "@/components/NetworkStatus"
import NetworkVisualizer from "@/components/NetworkVisualizer"
import { useAppContext } from "@/lib/context"

export default function HomePage() {
    const { mode, topology, rustStatus, webLLMProgress, webLLMError } = useAppContext()
    const [pitchMode, setPitchMode] = useState(false)
    const [toastMessage, setToastMessage] = useState<string | null>(null)

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
        <main className="app-shell" aria-label="Shard application shell">
            <Header />

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
                    topology={topology}
                    rustStatus={rustStatus}
                    webLLMProgress={webLLMProgress}
                    webLLMError={webLLMError}
                />
                <ChatPanel mode={mode} />
            </div>
        </main>
    )
}
