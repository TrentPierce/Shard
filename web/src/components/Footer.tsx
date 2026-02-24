"use client"

import Link from "next/link"
import { Github, Twitter, FileText, Zap } from "lucide-react"

const footerLinks: Record<string, { title: string; links: { name: string; href: string; external?: boolean }[] }> = {
    product: {
        title: "Product",
        links: [
            { name: "How It Works", href: "/#how-it-works" },
            { name: "Features", href: "/#features" },
            { name: "Use Cases", href: "/#use-cases" },
            { name: "API Reference", href: "/docs/API.md" },
        ],
    },
    developers: {
        title: "Developers",
        links: [
            { name: "Documentation", href: "/docs/API.md" },
            { name: "Quick Start", href: "/docs/deployment-guide.md" },
            { name: "GitHub", href: "https://github.com/TrentPierce/Shard", external: true },
            { name: "Contributing", href: "https://github.com/TrentPierce/Shard/blob/main/CONTRIBUTING.md", external: true },
        ],
    },
    resources: {
        title: "Resources",
        links: [
            { name: "White Paper", href: "https://github.com/TrentPierce/Shard/blob/main/docs/Shard-White-Paper-Feb-2026.md", external: true },
            { name: "Architecture", href: "https://github.com/TrentPierce/Shard/blob/main/ARCHITECTURE.md", external: true },
            { name: "Security", href: "https://github.com/TrentPierce/Shard/blob/main/SECURITY.md", external: true },
            { name: "Changelog", href: "https://github.com/TrentPierce/Shard/blob/main/CHANGELOG.md", external: true },
        ],
    },
}

export default function Footer() {
    const currentYear = new Date().getFullYear()

    return (
        <footer className="footer">
            <div className="container">
                <div className="footer-grid">
                    <div className="footer-brand">
                        <Link href="/" className="header-logo">
                            <svg viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true" style={{ width: 32, height: 32 }}>
                                <path d="M16 2L28 9V23L16 30L4 23V9L16 2Z" stroke="currentColor" strokeWidth="2" fill="none"/>
                                <path d="M16 2V16M16 16L28 9M16 16L4 9M16 16V30" stroke="currentColor" strokeWidth="2"/>
                                <circle cx="16" cy="16" r="4" fill="currentColor"/>
                            </svg>
                            <span>Shard</span>
                        </Link>
                        <p>
                            Browser-powered distributed inference. Free, unlimited LLM access through a decentralized P2P mesh.
                        </p>
                    </div>

                    {Object.entries(footerLinks).map(([key, section]) => (
                        <div key={key} className="footer-col">
                            <h4>{section.title}</h4>
                            <ul className="footer-links">
                                {section.links.map((link) => (
                                    <li key={link.name}>
                                        <a
                                            href={link.href}
                                            target={link.external ? "_blank" : undefined}
                                            rel={link.external ? "noopener noreferrer" : undefined}
                                        >
                                            {link.name}
                                        </a>
                                    </li>
                                ))}
                            </ul>
                        </div>
                    ))}
                </div>

                <div className="footer-bottom">
                    <p>
                        &copy; {currentYear} Shard Network. All rights reserved.
                    </p>
                    
                    <div className="footer-social">
                        <a href="https://github.com/TrentPierce/Shard" target="_blank" rel="noopener noreferrer" aria-label="GitHub">
                            <Github size={20} />
                        </a>
                        <a href="https://twitter.com/shardnetwork" target="_blank" rel="noopener noreferrer" aria-label="Twitter">
                            <Twitter size={20} />
                        </a>
                        <a href="/docs" aria-label="Documentation">
                            <FileText size={20} />
                        </a>
                    </div>
                </div>
            </div>
        </footer>
    )
}
