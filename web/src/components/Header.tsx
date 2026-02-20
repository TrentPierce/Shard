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

export default function Header({ mode, rustStatus }: HeaderProps) {
    const [isDesktop, setIsDesktop] = useState(false)

    useEffect(() => {
        if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
            setIsDesktop(true)
            document.body.classList.add("is-tauri")
        }
    }, [])

    const handleMinimize = async () => {
        const { getCurrentWindow } = await import("@tauri-apps/api/window")
        getCurrentWindow().minimize()
    }

    const handleClose = async () => {
        const { getCurrentWindow } = await import("@tauri-apps/api/window")
        getCurrentWindow().close()
    }

    return (
        <header className="header-modern" data-tauri-drag-region>
            <div className="header-modern__brand">
                <div className="header-modern__logo" aria-hidden="true">S</div>
                <div>
                    <h1>Shard</h1>
                    <p>Neural Mesh</p>
                </div>
            </div>

            <div className="header-modern__right">
                <div className="header-modern__status" aria-live="polite">
                    <span className={`header-modern__dot header-modern__dot--${rustStatus}`} />
                    <span>{rustStatus}</span>
                    <span className="header-modern__divider" />
                    <strong>{modeLabels[mode]}</strong>
                </div>
                <ThemeToggle />
                {isDesktop && (
                    <div className="header-modern__window-actions">
                        <button type="button" onClick={handleMinimize} aria-label="Minimize window">—</button>
                        <button type="button" onClick={handleClose} aria-label="Close window">×</button>
                    </div>
                )}
            </div>
        </header>
    )
}
