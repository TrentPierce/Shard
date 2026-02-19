"use client"

import { useEffect, useState } from "react"
import type { NodeMode } from "@/app/page"
import { ThemeToggle } from "./ThemeToggle"

interface HeaderProps {
    mode: NodeMode
    rustStatus: "connected" | "unreachable" | "downloading"
}

export const modeLabels: Record<NodeMode, string> = {
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

    return (
        <header className="h-16 border-b border-glass-border bg-glass-bg/80 backdrop-blur-xl px-8 flex items-center justify-between select-none z-[100] sticky top-0" data-tauri-drag-region>
            <div className="flex items-center gap-4 pointer-events-none">
                <div className="w-10 h-10 rounded-2xl bg-gradient-to-br from-accent-cyan to-accent-blue flex items-center justify-center text-primary font-bold shadow-glow-cyan text-xl">S</div>
                <div className="flex flex-col">
                    <h1 className="text-sm font-display font-black text-white tracking-widest uppercase italic">Shard</h1>
                    <span className="text-[10px] text-muted font-medium uppercase tracking-[0.3em] opacity-60">Neural Mesh v0.4.9</span>
                </div>
            </div>

            <div className="flex items-center gap-6">
                <div className="flex items-center gap-4 px-4 py-2 rounded-2xl bg-tertiary/40 border border-glass-border">
                    <div className="flex items-center gap-2">
                        <div className={`w-2 h-2 rounded-full animate-pulse shadow-sm ${rustStatus === "connected" ? "bg-accent-emerald shadow-accent-emerald/40" : rustStatus === "downloading" ? "bg-accent-amber shadow-accent-amber/40" : "bg-accent-rose shadow-accent-rose/40"}`}></div>
                        <span className="text-[10px] font-bold text-white uppercase tracking-wider">{rustStatus}</span>
                    </div>
                    <div className="h-3 w-[1px] bg-glass-border"></div>
                    <div className="text-[10px] font-bold text-accent-cyan uppercase tracking-wider tabular-nums">{modeLabels[mode]}</div>
                </div>

                <ThemeToggle />

                {isDesktop && (
                    <div className="flex items-center gap-2 ml-2">
                        <button
                            onClick={handleMinimize}
                            className="w-8 h-8 rounded-xl border border-glass-border bg-tertiary/40 text-muted hover:text-white hover:border-accent-cyan/40 flex items-center justify-center transition-smooth"
                        >
                            <svg width="12" height="2" viewBox="0 0 12 2" fill="currentColor">
                                <rect width="12" height="2" rx="1" />
                            </svg>
                        </button>
                        <button
                            onClick={handleClose}
                            className="w-8 h-8 rounded-xl border border-glass-border bg-tertiary/40 text-muted hover:text-accent-rose hover:border-accent-rose/40 flex items-center justify-center transition-smooth"
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
