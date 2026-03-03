"use client"

import Link from "next/link"
import { useCallback } from "react"

interface LandingPageProps {
    onEnter: () => void
}

const pillars = [
    {
        title: "Join as a browser Scout",
        body: "WebGPU-capable browsers can contribute draft tokens to verifier nodes in real time.",
    },
    {
        title: "Verifier-backed quality",
        body: "Verifier nodes score and validate drafts before tokens are finalized and streamed.",
    },
    {
        title: "Operational integration",
        body: "Overflow routing, SLA metrics, and Prometheus/Grafana support are built into the stack.",
    },
]

const roleCards = [
    { role: "Scout", desc: "Lightweight browser contributor for speculative drafts." },
    { role: "Verifier", desc: "Full node operator who validates and serves final tokens." },
    { role: "Builder", desc: "Product team integrating resilient AI endpoints." },
]

export default function LandingPage({ onEnter }: LandingPageProps) {
    const handleGetStarted = useCallback(() => onEnter(), [onEnter])

    return (
        <main className="landing-modern" aria-label="Shard landing page">
            <div className="landing-modern__bg" aria-hidden="true" />

            <section className="landing-modern__hero">
                <p className="landing-modern__eyebrow">Shard Network | Release 0.6.2</p>
                <h1>Distributed inference with production controls.</h1>
                <p className="landing-modern__lead">
                    Shard combines browser Scouts and verifier nodes into a fault-tolerant inference mesh with
                    bootstrap health checks, failover paths, and OpenAI-compatible APIs.
                </p>

                <div className="landing-modern__actions" style={{ position: "relative", zIndex: 10 }}>
                    <button
                        type="button"
                        className="landing-modern__btn landing-modern__btn--primary"
                        onClick={handleGetStarted}
                        style={{ cursor: "pointer" }}
                    >
                        Enter App
                    </button>
                    <Link className="landing-modern__btn landing-modern__btn--ghost" href="/leaderboard" style={{ cursor: "pointer" }}>
                        View Network Stats
                    </Link>
                </div>
            </section>

            <section className="landing-modern__panel" aria-label="Why Shard">
                <h2>Built for modern AI traffic</h2>
                <div className="landing-modern__grid">
                    {pillars.map((item) => (
                        <article key={item.title} className="landing-modern__card">
                            <h3>{item.title}</h3>
                            <p>{item.body}</p>
                        </article>
                    ))}
                </div>
            </section>

            <section className="landing-modern__panel" aria-label="Choose your role">
                <h2>Choose your role</h2>
                <div className="landing-modern__roles">
                    {roleCards.map((item) => (
                        <article key={item.role} className="landing-modern__role-card">
                            <h3>{item.role}</h3>
                            <p>{item.desc}</p>
                        </article>
                    ))}
                </div>
            </section>
        </main>
    )
}
