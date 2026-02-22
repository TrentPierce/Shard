"use client"

import { useEffect, useState } from "react"
import Link from "next/link"
import { usePathname } from "next/navigation"
import { useAppContext, type NodeMode } from "@/lib/context"
import { ThemeToggle } from "./ThemeToggle"

export const modeLabels: Record<NodeMode, string> = {
    loading: "Bootstrapping",
    "local-shard": "Shard",
    "scout-initializing": "Scout (Loading)",
    scout: "Scout",
    leech: "Consumer",
}

const navItems = [
    { name: "Chat", href: "/", icon: "💬" },
    { name: "Network", href: "/network", icon: "🌐" },
    { name: "Dashboard", href: "/dashboard", icon: "📊" },
]

export default function Header() {
    const { mode, rustStatus, theme } = useAppContext()
    const pathname = usePathname()
    const [isDesktop, setIsDesktop] = useState(false)

    const isEnterprise = theme === "enterprise"

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
        <header className="header" data-tauri-drag-region>
            <div className="app-container" style={{ display: 'flex', width: '100%', alignItems: 'center', justifyContent: 'space-between' }}>
                <div className="header__brand">
                    <div className="header__logo" aria-hidden="true">
                        {isEnterprise ? (
                            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-secondary">
                                <path d="M12 3l8 4.5v9L12 21l-8-4.5v-9L12 3z" />
                                <path d="M12 12l8-4.5M12 12v9M12 12L4 7.5" />
                            </svg>
                        ) : "[S]"}
                    </div>
                    <div>
                        <h1 className="header__title">
                            {isEnterprise ? "Shard Network" : "SHARD_TERMINAL"}
                        </h1>
                        <p className="header__subtitle">
                            {isEnterprise ? "Enterprise Distributed Inference" : "v0.4.9 // NEURAL_MESH_ESTABLISHED"}
                        </p>
                    </div>
                </div>

                <nav className="header-nav" style={{ display: 'flex', gap: '24px' }}>
                    {navItems.map((item) => {
                        const isActive = pathname === item.href
                        return (
                            <Link
                                key={item.name}
                                href={item.href}
                                className={`transition-colors text-[11px] font-bold uppercase tracking-[0.2em] relative py-1 ${isActive ? 'text-primary' : 'text-muted hover:text-primary'
                                    }`}
                            >
                                {!isEnterprise && isActive ? '> ' : ''}{item.name}
                                {isEnterprise && isActive && (
                                    <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-secondary" />
                                )}
                            </Link>
                        )
                    })}
                </nav>

                <div className="header-modern__right" style={{ display: 'flex', alignItems: 'center', gap: '20px' }}>
                    <div className="header__mode text-[10px] font-mono opacity-80" aria-live="polite">
                        STATUS: <span className={rustStatus === 'connected' ? 'text-[#33ff00]' : 'text-error'}>{rustStatus.toUpperCase()}</span>
                        <span style={{ margin: '0 10px', opacity: 0.3 }}>|</span>
                        MODE: <strong className="text-primary">{modeLabels[mode].toUpperCase()}</strong>
                    </div>

                    <ThemeToggle />

                    {isDesktop && (
                        <div className="header-modern__window-actions" style={{ display: 'flex', gap: '5px' }}>
                            <button type="button" className="btn-ping" style={{ padding: '2px 8px' }} onClick={handleMinimize} aria-label="Minimize window">_</button>
                            <button type="button" className="btn-ping" style={{ padding: '2px 8px', borderColor: 'var(--error)', color: 'var(--error)' }} onClick={handleClose} aria-label="Close window">X</button>
                        </div>
                    )}
                </div>
            </div>
        </header>
    )
}
