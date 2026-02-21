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
    const { mode, rustStatus } = useAppContext()
    const pathname = usePathname()
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
        <header className="header" data-tauri-drag-region>
            <div className="header__brand">
                <div className="header__logo" aria-hidden="true">[S]</div>
                <div>
                    <h1 className="header__title">SHARD_TERMINAL</h1>
                    <p className="header__subtitle">v0.4.9 // NEURAL_MESH_ESTABLISHED</p>
                </div>
            </div>

            <nav className="header-nav" style={{ display: 'flex', gap: '20px' }}>
                {navItems.map((item) => {
                    const isActive = pathname === item.href
                    return (
                        <Link
                            key={item.name}
                            href={item.href}
                            style={{
                                color: isActive ? 'var(--primary)' : 'var(--muted)',
                                textDecoration: 'none',
                                textTransform: 'uppercase',
                                fontSize: '12px',
                                letterSpacing: '2px',
                                borderBottom: isActive ? '2px solid var(--primary)' : 'none'
                            }}
                        >
                            {isActive ? '> ' : ''}{item.name}
                        </Link>
                    )
                })}
            </nav>

            <div className="header-modern__right" style={{ display: 'flex', alignItems: 'center', gap: '15px' }}>
                <div className="header__mode" aria-live="polite">
                    STATUS: <span className={`stat-value--${rustStatus === 'connected' ? 'accent' : 'error'}`}>{rustStatus}</span>
                    <span style={{ margin: '0 10px', opacity: 0.3 }}>|</span>
                    MODE: <strong>{modeLabels[mode]}</strong>
                </div>
                {isDesktop && (
                    <div className="header-modern__window-actions" style={{ display: 'flex', gap: '5px' }}>
                        <button type="button" className="btn-ping" style={{ padding: '2px 8px' }} onClick={handleMinimize} aria-label="Minimize window">_</button>
                        <button type="button" className="btn-ping" style={{ padding: '2px 8px', borderColor: 'var(--error)', color: 'var(--error)' }} onClick={handleClose} aria-label="Close window">X</button>
                    </div>
                )}
            </div>
        </header>
    )
}
