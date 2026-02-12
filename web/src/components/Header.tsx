"use client"

import type { NodeMode } from "@/app/page"
import { ThemeToggle } from "./ThemeToggle"

interface HeaderProps {
    mode: NodeMode
    rustStatus: "connected" | "unreachable"
}

const modeLabels: Record<NodeMode, string> = {
    loading: "Bootstrapping",
    "local-oracle": "Oracle",
    "scout-initializing": "Scout (Loading)",
    scout: "Scout",
    leech: "Consumer",
}

const modeDescriptions: Record<NodeMode, string> = {
    loading: "Connecting to network…",
    "local-oracle": "Full model loaded — verifying drafts",
    "scout-initializing": "Loading WebLLM model…",
    scout: "Contributing compute via WebGPU",
    leech: "Consuming inference — upgrade to Scout for priority",
}

export default function Header({ mode, rustStatus }: HeaderProps) {
    const dotClass =
        rustStatus === "connected" ? "status-dot--live" : "status-dot--dead"

    return (
        <header className="header" role="banner">
            <div className="header__brand" aria-label="Application brand">
                <div className="header__logo" aria-label="Shard logo" role="img">S</div>
                <div>
                    <div className="header__title">Shard</div>
                    <div className="header__subtitle">Distributed Inference Network</div>
                </div>
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: "16px" }} role="status" aria-live="polite">
                <div
                    className={`header__mode header__mode--${mode === "local-oracle" ? "oracle" : mode}`}
                    title={modeDescriptions[mode]}
                    role="status"
                >
                    <span className={`status-dot ${dotClass}`} aria-hidden="true" />
                    <span aria-label={`Current mode: ${modeLabels[mode]}`}>{modeLabels[mode]}</span>
                </div>
                <ThemeToggle />
            </div>
        </header>
    )
}
