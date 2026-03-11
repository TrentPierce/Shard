"use client"

import Link from "next/link"
import Image from "next/image"
import { Github, Twitter, FileText } from "lucide-react"
import { SHARD_VERSION } from "@/lib/version"

const footerLinks: Record<string, { title: string; links: { name: string; href: string; external?: boolean }[] }> = {
    product: {
        title: "Product",
        links: [
            { name: "How It Works", href: "/#how-it-works" },
            { name: "Features", href: "/#features" },
            { name: "Use Cases", href: "/#use-cases" },
            { name: "API Reference", href: "https://github.com/TrentPierce/Shard/blob/main/docs/api.md", external: true },
        ],
    },
    developers: {
        title: "Developers",
        links: [
            { name: "Documentation", href: "https://github.com/TrentPierce/Shard/tree/main/docs", external: true },
            { name: "Quick Start", href: "https://github.com/TrentPierce/Shard/blob/main/docs/deployment.md", external: true },
            { name: "GitHub", href: "https://github.com/TrentPierce/Shard", external: true },
            { name: "Contributing", href: "https://github.com/TrentPierce/Shard/blob/main/docs/contributing.md", external: true },
        ],
    },
    resources: {
        title: "Resources",
        links: [
            { name: "Architecture", href: "https://github.com/TrentPierce/Shard/blob/main/docs/architecture.md", external: true },
            { name: "Versioning", href: "https://github.com/TrentPierce/Shard/blob/main/docs/versioning.md", external: true },
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
                            <Image src="/brand-mark.png" alt="" width={32} height={32} aria-hidden="true" />
                            <span>Shard</span>
                        </Link>
                        <p>
                            Local-first browser AI with desktop verifier routing. Experimental WAN scout paths stay available for benchmark work, not the default product flow.
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
                        &copy; {currentYear} Shard Network. All rights reserved. v{SHARD_VERSION}
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
