"use client"

import Link from "next/link"
import { useCallback } from "react"

interface LandingPageProps {
    onEnter: () => void
}

const pillars = [
    {
        title: "Answer locally first",
        body: "Simple prompts can complete directly in the browser for zero-hop latency and private-by-default UX.",
    },
    {
        title: "Escalate when it matters",
        body: "Harder requests route to desktop verifier nodes that keep heavyweight models hot and ready.",
    },
    {
        title: "Classify low-power contributors",
        body: "Browsers now distinguish active WebGPU runtime support from future WebNN/NPU eligibility so Shard can grow a quieter background lane without changing the shipping path yet.",
    },
]

const roleCards = [
    { role: "Browser", desc: "Local-first runtime that answers easy prompts and routes the rest." },
    { role: "Verifier", desc: "Desktop heavy-inference worker for escalated requests and local speculative decode." },
    { role: "Builder", desc: "Product team integrating resilient AI endpoints without central lock-in." },
]

export default function LandingPage({ onEnter }: LandingPageProps) {
    const handleGetStarted = useCallback(() => onEnter(), [onEnter])

    return (
        <main className="landing-modern" aria-label="Shard landing page">
            <div className="landing-modern__bg" aria-hidden="true" />

            <section className="landing-modern__hero">
                <p className="landing-modern__eyebrow">Shard Network | Release 0.6.6</p>
                <h1>Local-first AI routing with desktop heavy inference.</h1>
                <p className="landing-modern__lead">
                    Shard answers easy prompts in the browser, escalates harder requests to verifier nodes, and keeps
                    experimental WAN scout workflows available for benchmark research only while it prepares a separate low-power browser contributor lane.
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
