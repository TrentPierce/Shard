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

            <section className="landing__hero">
                <div className="landing__logo-ring" aria-hidden="true">
                    <div className="landing__logo-glow" />
                    <div className="landing__logo-text">S</div>
                </div>
                <h1 className="landing__title">Shard</h1>
                <p className="landing__tagline">Distributed Inference, Operated by Contributors</p>
                <p className="landing__subtitle">
                    Run as a Scout in the browser, run a full Shard node on a server,
                    or consume the API. The network is strongest when all three roles
                    are active.
                </p>

                <div className="landing__cta-group">
                    <button className="landing__cta" onClick={handleGetStarted}>
                        <span className="landing__cta-text">Enter App</span>
                        <span className="landing__cta-arrow">-&gt;</span>
                    </button>
                    <a className="landing__cta landing__cta--secondary" href="/network">
                        <span className="landing__cta-text">View Leaderboard</span>
                        <span className="landing__cta-arrow">-&gt;</span>
                    </a>
                </div>

                <p className="landing__note">
                    Single-node mode works. Distributed mode unlocks higher throughput,
                    lower tail latency, and better resilience.
                </p>
            </section>

            <section className="landing__section">
                <h2 className="landing__section-title">How Contribution Works</h2>
                <div className="landing__steps">
                    <div className="landing__step">
                        <div className="landing__step-number">1</div>
                        <div className="landing__step-icon">JOIN</div>
                        <h3>Join as Scout</h3>
                        <p>
                            Browser nodes draft token candidates. This contributes
                            speculative compute to active requests.
                        </p>
                    </div>
                    <div className="landing__step-connector" aria-hidden="true" />
                    <div className="landing__step">
                        <div className="landing__step-number">2</div>
                        <div className="landing__step-icon">VERIFY</div>
                        <h3>Shard Verifies</h3>
                        <p>
                            Full-model Shard nodes accept or reject drafts and
                            produce final tokens with deterministic verification.
                        </p>
                    </div>
                    <div className="landing__step-connector" aria-hidden="true" />
                    <div className="landing__step">
                        <div className="landing__step-number">3</div>
                        <div className="landing__step-icon">SCALE</div>
                        <h3>Network Scales</h3>
                        <p>
                            More Scouts increase draft supply. More Shards increase
                            verified throughput and reliability.
                        </p>
                    </div>
                </div>
            </section>

            <section className="landing__section">
                <h2 className="landing__section-title">Choose Your Role</h2>
                <div className="landing__role-grid">
                    <div className="landing__card">
                        <div className="landing__card-icon">SCOUT</div>
                        <h3>Browser Scout</h3>
                        <p>Runs lightweight drafts. Easiest way to contribute in seconds.</p>
                    </div>
                    <div className="landing__card">
                        <div className="landing__card-icon">SHARD</div>
                        <h3>Verifier Node</h3>
                        <p>Runs full model verification. Adds capacity and fault tolerance.</p>
                    </div>
                    <div className="landing__card">
                        <div className="landing__card-icon">API</div>
                        <h3>Client Consumer</h3>
                        <p>Uses OpenAI-compatible endpoints and benefits from network growth.</p>
                    </div>
                </div>
            </section>

            <section className="landing__section">
                <h2 className="landing__section-title">Single Node vs Network</h2>
                <div className="landing__cards">
                    <div className="landing__card">
                        <div className="landing__card-icon">LOCAL</div>
                        <h3>Single-Node Mode</h3>
                        <p>
                            Works with one EC2 Shard. Reliable baseline, but limited throughput
                            and no distributed fault tolerance.
                        </p>
                    </div>
                    <div className="landing__card">
                        <div className="landing__card-icon">MESH</div>
                        <h3>Distributed Mode</h3>
                        <p>
                            Scouts accelerate drafts and multiple Shards absorb load.
                            Better tail latency and resilience under traffic spikes.
                        </p>
                    </div>
                </div>
            </section>

            <section className="landing__section">
                <h2 className="landing__section-title">Built With</h2>
                <div className="landing__tech-grid">
                    <div className="landing__tech">
                        <span className="landing__tech-name">GGUF Runtime</span>
                        <span className="landing__tech-desc">Local verifier models</span>
                    </div>
                    <div className="landing__tech">
                        <span className="landing__tech-name">libp2p</span>
                        <span className="landing__tech-desc">P2P networking</span>
                    </div>
                    <div className="landing__tech">
                        <span className="landing__tech-name">WebLLM</span>
                        <span className="landing__tech-desc">Scout draft generation</span>
                    </div>
                    <div className="landing__tech">
                        <span className="landing__tech-name">WebGPU</span>
                        <span className="landing__tech-desc">Browser acceleration</span>
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

            <section className="landing__footer">
                <p className="landing__footer-text">Ready to contribute?</p>
                <div className="landing__cta-group">
                    <button className="landing__cta landing__cta--secondary" onClick={handleGetStarted}>
                        <span className="landing__cta-text">Launch App</span>
                        <span className="landing__cta-arrow">-&gt;</span>
                    </button>
                    <a
                        className="landing__cta landing__cta--secondary"
                        href="https://github.com/TrentPierce/Shard/blob/main/docs/deployment-guide.md"
                        target="_blank"
                        rel="noopener noreferrer"
                    >
                        <span className="landing__cta-text">Run a Shard Node</span>
                        <span className="landing__cta-arrow">-&gt;</span>
                    </a>
                </div>
                <div className="landing__links">
                    <a href="https://github.com/TrentPierce/Shard" target="_blank" rel="noopener noreferrer">
                        GitHub
                    </a>
                    <span className="landing__link-sep">-</span>
                    <a href="https://github.com/TrentPierce/Shard/tree/main/docs" target="_blank" rel="noopener noreferrer">
                        Docs
                    </a>
                    <span className="landing__link-sep">-</span>
                    <a href="https://github.com/TrentPierce/Shard/blob/main/docs/Shard-White-Paper-Feb-2026.pdf" target="_blank" rel="noopener noreferrer">
                        White Paper
                    </a>
                    <span className="landing__link-sep">-</span>
                    <a href="https://github.com/TrentPierce/Shard/blob/main/docs/API.md" target="_blank" rel="noopener noreferrer">
                        API
                    </a>
                </div>
            </section>
        </div>
    )
}
