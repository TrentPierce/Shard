"use client"

import Link from "next/link"
import { usePathname } from "next/navigation"

import { MessageSquare, Globe, LayoutDashboard } from "lucide-react"

const navItems = [
    { name: "Chat", href: "/", icon: <MessageSquare size={18} /> },
    { name: "Network", href: "/network", icon: <Globe size={18} /> },
    { name: "Dashboard", href: "/dashboard", icon: <LayoutDashboard size={18} /> },
]

export default function Navigation() {
    const pathname = usePathname()

    return (
        <nav className="main-nav glass-card">
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
                    padding: 8px;
                    border: none;
                    border-bottom: var(--border-width) solid var(--glass-border);
                    background: var(--glass-bg);
                    backdrop-filter: blur(12px);
                    border-radius: 0;
                }
                .main-nav__list {
                    display: flex;
                    gap: 4px;
                    justify-content: center;
                }
                .main-nav__item {
                    display: flex;
                    align-items: center;
                    gap: 10px;
                    padding: 8px 16px;
                    border-radius: var(--radius-inner);
                    text-decoration: none;
                    color: var(--text-secondary);
                    font-size: 13px;
                    font-weight: 500;
                    transition: all var(--transition);
                    border: var(--border-width) solid transparent;
                }
                .main-nav__item:hover {
                    background: var(--glass-bg);
                    color: var(--text-primary);
                    border-color: var(--glass-border);
                }
                .main-nav__item--active {
                    background: rgba(var(--primary), 0.1);
                    color: var(--primary);
                    border-color: var(--primary);
                    font-weight: 600;
                }
                .theme-enterprise .main-nav__item--active {
                    background: rgba(0, 112, 243, 0.1);
                    color: #0070f3;
                    border-color: rgba(0, 112, 243, 0.2);
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
                        padding: 10px;
                    }
                }
            `}</style>
        </nav>
    )
}
