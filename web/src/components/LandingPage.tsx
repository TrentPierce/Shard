"use client"

import { useCallback } from "react"

interface LandingPageProps {
    onEnter: () => void
}

export default function LandingPage({ onEnter }: LandingPageProps) {
    const handleGetStarted = useCallback(() => {
        onEnter()
    }, [onEnter])

    return (
        <div className="landing">
            {/* Animated background particles */}
            <div className="landing__particles" aria-hidden="true">
                {Array.from({ length: 20 }).map((_, i) => (
                    <div
                        key={i}
                        className="landing__particle"
                        style={{
                            left: `${Math.random() * 100}%`,
                            top: `${Math.random() * 100}%`,
                            animationDelay: `${Math.random() * 5}s`,
                            animationDuration: `${3 + Math.random() * 4}s`,
                        }}
                    />
                ))}
            </div>

            {/* Hero */}
            <section className="landing__hero">
                <div className="landing__logo-ring" aria-hidden="true">
                    <div className="landing__logo-glow" />
                    <div className="landing__logo-text">S</div>
                </div>
                <h1 className="landing__title">Shard</h1>
                <p className="landing__tagline">
                    Browser-Powered Distributed Inference
                </p>
                <p className="landing__subtitle">
                    Free, unlimited LLM access through a decentralized P2P mesh.
                    <br />
                    Your browser becomes an AI compute node.
                </p>
                <button className="landing__cta" onClick={handleGetStarted}>
                    <span className="landing__cta-text">Start Contributing</span>
                    <span className="landing__cta-arrow">→</span>
                </button>
            </section>

            {/* How It Works */}
            <section className="landing__section">
                <h2 className="landing__section-title">How It Works</h2>
                <div className="landing__steps">
                    <div className="landing__step">
                        <div className="landing__step-number">1</div>
                        <div className="landing__step-icon">🌐</div>
                        <h3>Open the App</h3>
                        <p>
                            Your browser loads a lightweight AI model via WebGPU.
                            No install needed.
                        </p>
                    </div>
                    <div className="landing__step-connector" aria-hidden="true" />
                    <div className="landing__step">
                        <div className="landing__step-number">2</div>
                        <div className="landing__step-icon">⚡</div>
                        <h3>Become a Scout</h3>
                        <p>
                            Your browser joins the P2P mesh and drafts tokens
                            for AI requests across the network.
                        </p>
                    </div>
                    <div className="landing__step-connector" aria-hidden="true" />
                    <div className="landing__step">
                        <div className="landing__step-number">3</div>
                        <div className="landing__step-icon">✅</div>
                        <h3>Network Verifies</h3>
                        <p>
                            Shard nodes run full models to verify drafts in one
                            parallel pass — server-grade quality at zero cost.
                        </p>
                    </div>
                </div>
            </section>

            {/* Value Proposition */}
            <section className="landing__section">
                <h2 className="landing__section-title">Why Shard?</h2>
                <div className="landing__cards">
                    <div className="landing__card">
                        <div className="landing__card-icon">💰</div>
                        <h3>Free Access</h3>
                        <p>No API keys, no token limits. Contribute compute, earn priority.</p>
                    </div>
                    <div className="landing__card">
                        <div className="landing__card-icon">🔒</div>
                        <h3>Privacy First</h3>
                        <p>Localhost-first routing. Your data never touches a corporate server.</p>
                    </div>
                    <div className="landing__card">
                        <div className="landing__card-icon">📈</div>
                        <h3>Scales With Users</h3>
                        <p>More visitors = more GPU power. The mesh grows with demand.</p>
                    </div>
                    <div className="landing__card">
                        <div className="landing__card-icon">🔌</div>
                        <h3>OpenAI Compatible</h3>
                        <p>Drop-in replacement API. Use any existing client library.</p>
                    </div>
                </div>
            </section>

            {/* Tech Stack */}
            <section className="landing__section">
                <h2 className="landing__section-title">Built With</h2>
                <div className="landing__tech-grid">
                    <div className="landing__tech">
                        <span className="landing__tech-name">BitNet 1.58b</span>
                        <span className="landing__tech-desc">Ternary quantization</span>
                    </div>
                    <div className="landing__tech">
                        <span className="landing__tech-name">libp2p</span>
                        <span className="landing__tech-desc">P2P networking</span>
                    </div>
                    <div className="landing__tech">
                        <span className="landing__tech-name">WebLLM</span>
                        <span className="landing__tech-desc">Browser inference</span>
                    </div>
                    <div className="landing__tech">
                        <span className="landing__tech-name">WebGPU</span>
                        <span className="landing__tech-desc">GPU compute</span>
                    </div>
                    <div className="landing__tech">
                        <span className="landing__tech-name">Rust</span>
                        <span className="landing__tech-desc">P2P daemon</span>
                    </div>
                    <div className="landing__tech">
                        <span className="landing__tech-name">FastAPI</span>
                        <span className="landing__tech-desc">Inference API</span>
                    </div>
                </div>
            </section>

            {/* Footer CTA */}
            <section className="landing__footer">
                <p className="landing__footer-text">
                    Ready to join the mesh?
                </p>
                <button className="landing__cta landing__cta--secondary" onClick={handleGetStarted}>
                    <span className="landing__cta-text">Launch App</span>
                    <span className="landing__cta-arrow">→</span>
                </button>
                <div className="landing__links">
                    <a href="https://github.com/TrentPierce/Shard" target="_blank" rel="noopener noreferrer">
                        GitHub
                    </a>
                    <span className="landing__link-sep">·</span>
                    <a href="https://github.com/TrentPierce/Shard/tree/main/docs" target="_blank" rel="noopener noreferrer">
                        Docs
                    </a>
                    <span className="landing__link-sep">·</span>
                    <a href="https://github.com/TrentPierce/Shard/blob/main/docs/Shard-White-Paper-Feb-2026.pdf" target="_blank" rel="noopener noreferrer">
                        White Paper
                    </a>
                    <span className="landing__link-sep">·</span>
                    <a href="https://github.com/TrentPierce/Shard/blob/main/docs/API.md" target="_blank" rel="noopener noreferrer">
                        API
                    </a>
                </div>
            </section>
        </div>
    )
}
