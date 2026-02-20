"use client"

import { useCallback } from "react"

interface LandingPageProps {
    onEnter: () => void
}

const pillars = [
    {
        title: "Contribute in 30 seconds",
        body: "Join as a browser Scout with WebGPU and start drafting tokens for live requests.",
    },
    {
        title: "Deterministic verification",
        body: "Shard verifier nodes validate drafts and finalize outputs with reproducible checks.",
    },
    {
        title: "OpenAI-compatible API",
        body: "Plug existing apps into a distributed inference mesh without rewrites.",
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
                <p className="landing-modern__eyebrow">Shard Network • 2026-ready architecture</p>
                <h1>Distributed inference that feels instant.</h1>
                <p className="landing-modern__lead">
                    Shard combines browser Scouts and verifier nodes into a high-throughput, fault-tolerant AI mesh.
                    Contribute compute, scale globally, and ship with predictable latency.
                </p>

                <div className="landing-modern__actions">
                    <button type="button" className="landing-modern__btn landing-modern__btn--primary" onClick={handleGetStarted}>
                        Enter App
                    </button>
                    <a className="landing-modern__btn landing-modern__btn--ghost" href="/network">
                        View Live Network
                    </a>
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
