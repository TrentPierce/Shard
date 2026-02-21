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
        <header className="header-modern" data-tauri-drag-region>
            <div className="header-modern__brand">
                <div className="header-modern__logo" aria-hidden="true">S</div>
                <div>
                    <h1>Shard</h1>
                    <p>Neural Mesh</p>
                </div>
            </div>

            <nav className="header-nav">
                {navItems.map((item) => {
                    const isActive = pathname === item.href
                    return (
                        <Link
                            key={item.name}
                            href={item.href}
                            className={`header-nav__item ${isActive ? "header-nav__item--active" : ""}`}
                        >
                            <span className="header-nav__icon">{item.icon}</span>
                            <span className="header-nav__name">{item.name}</span>
                        </Link>
                    )
                })}
            </nav>

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
            <style jsx>{`
                .header-nav {
                    display: flex;
                    gap: 4px;
                    background: rgba(255, 255, 255, 0.03);
                    padding: 4px;
                    border-radius: var(--radius-lg);
                    border: 1px solid var(--glass-border);
                }
                :global([data-theme="light"]) .header-nav {
                    background: rgba(255, 255, 255, 0.88);
                    border-color: rgba(15, 23, 42, 0.14);
                }
                .header-nav__item {
                    display: flex;
                    align-items: center;
                    gap: 8px;
                    padding: 6px 14px;
                    border-radius: var(--radius-md);
                    text-decoration: none;
                    color: var(--text-secondary);
                    font-size: 13px;
                    font-weight: 600;
                    transition: all var(--transition-fast);
                }
                .header-nav__item:hover {
                    color: var(--text-primary);
                    background: rgba(255, 255, 255, 0.05);
                }
                .header-nav__item:focus-visible {
                    outline: 2px solid var(--accent-blue);
                    outline-offset: 2px;
                }
                :global([data-theme="light"]) .header-nav__item {
                    color: #334155;
                }
                :global([data-theme="light"]) .header-nav__item:hover {
                    color: #0f172a;
                    background: rgba(37, 99, 235, 0.1);
                }
                .header-nav__item--active {
                    color: var(--accent-cyan);
                    background: rgba(56, 139, 180, 0.12);
                }
                :global([data-theme="light"]) .header-nav__item--active {
                    color: #1d4ed8;
                    background: rgba(37, 99, 235, 0.14);
                }
                @media (max-width: 900px) {
                    .header-nav__name {
                        display: none;
                    }
                    .header-nav__item {
                        padding: 6px;
                    }
                }
            `}</style>
        </header>
    )
}
