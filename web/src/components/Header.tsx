"use client"

import { useEffect, useState } from "react"
import type { NodeMode } from "@/app/page"
import { ThemeToggle } from "./ThemeToggle"

interface HeaderProps {
    mode: NodeMode
    rustStatus: "connected" | "unreachable"
}

const modeLabels: Record<NodeMode, string> = {
    loading: "Bootstrapping",
    "local-shard": "Shard",
    "scout-initializing": "Scout (Loading)",
    scout: "Scout",
    leech: "Consumer",
}

const modeDescriptions: Record<NodeMode, string> = {
    loading: "Connecting to network…",
    "local-shard": "Full model loaded — verifying drafts",
    "scout-initializing": "Loading WebLLM model…",
    scout: "Contributing compute via WebGPU",
    leech: "Consuming inference — upgrade to Scout for priority",
}

export default function Header({ mode, rustStatus }: HeaderProps) {
    const [isDesktop, setIsDesktop] = useState(false)

    useEffect(() => {
        if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
            setIsDesktop(true)
            document.body.classList.add("is-tauri")
        }
    }, [])

    const handleMinimize = async () => {
        const { getCurrentWindow } = await import('@tauri-apps/api/window')
        getCurrentWindow().minimize()
    }

    const handleClose = async () => {
        const { getCurrentWindow } = await import('@tauri-apps/api/window')
        getCurrentWindow().close()
    }

    const dotClass =
        rustStatus === "connected" ? "status-dot--live" : "status-dot--dead"

    return (
        <header className="header" role="banner" data-tauri-drag-region>
            <div className="header__brand" aria-label="Application brand" style={{ pointerEvents: 'none' }}>
                <div className="header__logo" aria-label="Shard logo" role="img">S</div>
                <div>
                    <div className="header__title">Shard</div>
                    <div className="header__subtitle">Distributed Inference Network</div>
                </div>
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: "16px", zIndex: 10 }}>
                <div
                    className={`header__mode header__mode--${mode === "local-shard" ? "shard" : mode}`}
                    title={modeDescriptions[mode]}
                    role="status"
                >
                    <span className={`status-dot ${dotClass}`} aria-hidden="true" />
                    <span aria-label={`Current mode: ${modeLabels[mode]}`}>{modeLabels[mode]}</span>
                </div>
                <ThemeToggle />
                {isDesktop && (
                    <div style={{ display: "flex", gap: "8px", marginLeft: "12px" }}>
                        <button
                            onClick={handleMinimize}
                            title="Minimize"
                            style={{
                                width: "28px", height: "28px", borderRadius: "50%",
                                border: "1px solid rgba(148,163,184,0.3)", background: "var(--bg-card)",
                                color: "var(--text-muted)", cursor: "pointer", display: "flex",
                                alignItems: "center", justifyContent: "center", fontSize: "16px",
                                transition: "all 0.2s ease"
                            }}
                            onMouseEnter={(e) => { e.currentTarget.style.color = "var(--text-primary)"; e.currentTarget.style.borderColor = "var(--border-glow)"; }}
                            onMouseLeave={(e) => { e.currentTarget.style.color = "var(--text-muted)"; e.currentTarget.style.borderColor = "rgba(148,163,184,0.3)"; }}
                        >
                            <svg width="12" height="2" viewBox="0 0 12 2" fill="currentColor">
                                <rect width="12" height="2" rx="1" />
                            </svg>
                        </button>
                        <button
                            onClick={handleClose}
                            title="Minimize to Tray"
                            style={{
                                width: "28px", height: "28px", borderRadius: "50%",
                                border: "1px solid rgba(148,163,184,0.3)", background: "var(--bg-card)",
                                color: "var(--text-muted)", cursor: "pointer", display: "flex",
                                alignItems: "center", justifyContent: "center", fontSize: "16px",
                                transition: "all 0.2s ease"
                            }}
                            onMouseEnter={(e) => { e.currentTarget.style.color = "var(--accent-rose)"; e.currentTarget.style.borderColor = "var(--accent-rose)"; }}
                            onMouseLeave={(e) => { e.currentTarget.style.color = "var(--text-muted)"; e.currentTarget.style.borderColor = "rgba(148,163,184,0.3)"; }}
                        >
                            <svg width="10" height="10" viewBox="0 0 12 12" fill="currentColor">
                                <path d="M10.7 1.3c-.4-.4-1-.4-1.4 0L6 4.6 2.7 1.3c-.4-.4-1-.4-1.4 0-.4.4-.4 1 0 1.4L4.6 6 1.3 9.3c-.4.4-.4 1 0 1.4.2.2.5.3.7.3.2 0 .5-.1.7-.3L6 7.4l3.3 3.3c.2.2.5.3.7.3.2 0 .5-.1.7-.3.4-.4.4-1 0-1.4L7.4 6l3.3-3.3c.4-.4.4-1 0-1.4z" />
                            </svg>
                        </button>
                    </div>
                )}
            </div>
        </header>
    )
}
