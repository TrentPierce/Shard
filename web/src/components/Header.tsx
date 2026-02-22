"use client"

import { useState } from "react"
import Link from "next/link"
import { Menu, X, ChevronRight } from "lucide-react"

export const modeLabels: Record<string, string> = {
    loading: "Bootstrapping",
    "local-shard": "Shard",
    "scout-initializing": "Scout (Loading)",
    scout: "Scout",
    leech: "Consumer",
}

const navLinks = [
    { name: "How It Works", href: "#how-it-works" },
    { name: "Features", href: "#features" },
    { name: "Use Cases", href: "#use-cases" },
    { name: "API", href: "#api" },
    { name: "Docs", href: "/docs", external: true },
]

export default function Header() {
    const [mobileMenuOpen, setMobileMenuOpen] = useState(false)

    return (
        <header className="header">
            <div className="header-inner">
                <Link href="/" className="header-logo" aria-label="Shard Home">
                    <svg viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
                        <path d="M16 2L28 9V23L16 30L4 23V9L16 2Z" stroke="currentColor" strokeWidth="2" fill="none"/>
                        <path d="M16 2V16M16 16L28 9M16 16L4 9M16 16V30" stroke="currentColor" strokeWidth="2"/>
                        <circle cx="16" cy="16" r="4" fill="currentColor"/>
                    </svg>
                    <span>Shard</span>
                </Link>

                <nav className="header-nav" aria-label="Main navigation">
                    {navLinks.map((link) => (
                        <a
                            key={link.name}
                            href={link.href}
                            className="header-nav-link"
                            target={link.external ? "_blank" : undefined}
                            rel={link.external ? "noopener noreferrer" : undefined}
                        >
                            {link.name}
                            {link.external && <ChevronRight size={12} style={{ marginLeft: 4, opacity: 0.5 }} />}
                        </a>
                    ))}
                </nav>

                <div className="header-actions">
                    <Link href="https://github.com/TrentPierce/Shard" className="btn btn-secondary btn-sm" target="_blank" rel="noopener noreferrer">
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                            <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
                        </svg>
                        GitHub
                    </Link>
                    
                    <button
                        className="mobile-menu-btn"
                        onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
                        aria-expanded={mobileMenuOpen}
                        aria-label={mobileMenuOpen ? "Close menu" : "Open menu"}
                    >
                        {mobileMenuOpen ? <X size={24} /> : <Menu size={24} />}
                    </button>
                </div>
            </div>

            {/* Mobile Menu */}
            {mobileMenuOpen && (
                <div className="mobile-menu" style={{
                    position: 'absolute',
                    top: '100%',
                    left: 0,
                    right: 0,
                    background: 'var(--bg-secondary)',
                    borderBottom: '1px solid var(--border)',
                    padding: 'var(--space-lg)',
                    zIndex: 100
                }}>
                    <nav className="flex flex-col gap-md">
                        {navLinks.map((link) => (
                            <a
                                key={link.name}
                                href={link.href}
                                className="header-nav-link"
                                style={{ padding: 'var(--space-sm) 0' }}
                                target={link.external ? "_blank" : undefined}
                                rel={link.external ? "noopener noreferrer" : undefined}
                                onClick={() => setMobileMenuOpen(false)}
                            >
                                {link.name}
                            </a>
                        ))}
                    </nav>
                </div>
            )}
        </header>
    )
}
