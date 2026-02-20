"use client"

import Link from "next/link"
import { usePathname } from "next/navigation"

const navItems = [
    { name: "Chat", href: "/", icon: "💬" },
    { name: "Network", href: "/network", icon: "🌐" },
    { name: "Dashboard", href: "/dashboard", icon: "📊" },
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
                    border-bottom: 1px solid var(--glass-border);
                    background: var(--glass-bg);
                    backdrop-filter: blur(var(--glass-blur));
                }
                .main-nav__list {
                    display: flex;
                    gap: 8px;
                    justify-content: center;
                }
                .main-nav__item {
                    display: flex;
                    align-items: center;
                    gap: 8px;
                    padding: 8px 16px;
                    border-radius: var(--radius-md);
                    text-decoration: none;
                    color: var(--text-secondary);
                    font-size: 14px;
                    font-weight: 600;
                    transition: all var(--transition-fast);
                    border: 1px solid transparent;
                }
                .main-nav__item:hover {
                    background: rgba(255, 255, 255, 0.05);
                    color: var(--text-primary);
                }
                .main-nav__item--active {
                    background: rgba(56, 139, 180, 0.1);
                    color: var(--accent-cyan);
                    border-color: rgba(56, 139, 180, 0.2);
                }
                .main-nav__icon {
                    font-size: 18px;
                }
                @media (max-width: 768px) {
                    .main-nav__name {
                        display: none;
                    }
                    .main-nav__item {
                        padding: 8px;
                    }
                }
            `}</style>
        </nav>
    )
}
