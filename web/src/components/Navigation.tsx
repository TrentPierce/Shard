"use client"

import Link from "next/link"
import { usePathname } from "next/navigation"

import { MessageSquare, Globe, LayoutDashboard, Trophy } from "lucide-react"

const navItems = [
    { name: "Chat", href: "/chat", icon: <MessageSquare size={18} /> },
    { name: "Network", href: "/network", icon: <Globe size={18} /> },
    { name: "Leaderboard", href: "/leaderboard", icon: <Trophy size={18} /> },
    { name: "Dashboard", href: "/dashboard", icon: <LayoutDashboard size={18} /> },
]

export default function Navigation() {
    const pathname = usePathname()

    return (
        <nav className="main-nav">
            <div className="main-nav__list">
                {navItems.map((item) => {
                    const isActive = pathname === item.href
                    return (
                        <Link
                            key={item.name}
                            href={item.href}
                            className={`main-nav__item ${isActive ? "main-nav__item--active" : ""}`}
                        >
                            <span className="main-nav__icon">{item.icon}</span>
                            <span className="main-nav__name">{item.name}</span>
                        </Link>
                    )
                })}
            </div>
            <style jsx>{`
                .main-nav {
                    padding: 12px;
                    border-bottom: 1px solid var(--border);
                    background: rgba(10, 10, 15, 0.8);
                    backdrop-filter: blur(12px);
                }
                .main-nav__list {
                    display: flex;
                    gap: 8px;
                    justify-content: center;
                    max-width: var(--max-width);
                    margin: 0 auto;
                }
                .main-nav__item {
                    display: flex;
                    align-items: center;
                    gap: 10px;
                    padding: 8px 20px;
                    border-radius: var(--radius-md);
                    text-decoration: none;
                    color: var(--text-secondary);
                    font-size: 14px;
                    font-weight: 600;
                    transition: all var(--transition-fast);
                    border: 1px solid transparent;
                }
                .main-nav__item:hover {
                    background: var(--bg-tertiary);
                    color: var(--text-primary);
                }
                .main-nav__item--active {
                    background: var(--primary-glow);
                    color: var(--primary);
                    border-color: rgba(0, 212, 170, 0.2);
                }
                .main-nav__icon {
                    display: flex;
                    align-items: center;
                }
                @media (max-width: 768px) {
                    .main-nav__name {
                        display: none;
                    }
                    .main-nav__item {
                        padding: 12px;
                    }
                }
            `}</style>
        </nav>
    )
}
