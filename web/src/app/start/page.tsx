"use client"

import Link from "next/link"
import { useMemo } from "react"
import { useAppContext } from "@/lib/context"

type PathCard = {
  id: "provenance" | "desktop" | "api"
  title: string
  summary: string
  action: string
  href: string
  steps: string[]
  notes: string[]
}

const paths: PathCard[] = [
  {
    id: "provenance",
    title: "Provenance Demo",
    summary:
      "The clearest way to understand Shard V1. Run the `research_brief` workflow and inspect the receipt chain immediately.",
    action: "Open provenance demo",
    href: "/provenance",
    steps: [
      "Provide a research question and source bundle.",
      "Set supply tiers, trust floor, and budget constraints.",
      "Review the execution summary, receipts, and provenance graph.",
    ],
    notes: [
      "This is the flagship v1 surface and the best place to evaluate Shard’s differentiation.",
      "The graph remains useful even when a step fails or falls back to a less desirable route.",
    ],
  },
  {
    id: "desktop",
    title: "Personal or Private Capacity",
    summary:
      "Run Shard on a spare PC or team machine so agents can prefer your own capacity before they fall back to public supply.",
    action: "Download Shard GUI",
    href: "https://github.com/TrentPierce/Shard/releases/latest",
    steps: [
      "Download and open Shard GUI.",
      "Let the model finish downloading on first run.",
      "Save settings, restart the node once, then click Start.",
    ],
    notes: [
      "Use this when you want local ownership over latency, trust, and data placement.",
      "Healthy nodes can serve personal, private, or public work depending on policy.",
    ],
  },
  {
    id: "api",
    title: "API + SDK Integration",
    summary:
      "Use chat as the compatibility baseline, then adopt task and provenance APIs when you need workflow-level observability.",
    action: "Read API docs",
    href: "https://github.com/TrentPierce/Shard/blob/main/docs/api.md",
    steps: [
      "Start with `/v1/chat/completions` if you need a familiar surface.",
      "Adopt `POST /v1/agents/tasks` for the `research_brief` workflow.",
      "Fetch receipts and provenance graphs to debug routing and fallback behavior.",
    ],
    notes: [
      "The Python SDK exposes `client.agents.submit/status/receipts/provenance/capabilities`.",
      "Use this path if you are building agent workflows rather than just testing the UI.",
    ],
  },
]

export default function StartPage() {
  const { contributionStatus } = useAppContext()
  const statusText = useMemo(() => {
    if (!contributionStatus) return null
    if (contributionStatus.state === "contributing") return "Contributing"
    if (contributionStatus.state === "starting") return "Starting"
    return "Not contributing"
  }, [contributionStatus])

  return (
    <main id="main-content" className="py-8 sm:py-10">
      <section className="rounded-[2rem] border border-ring bg-base-800 p-6 shadow-panel sm:p-8">
        <p className="text-xs uppercase tracking-[0.22em] text-ink-400">Quick Start</p>
        <h1 className="mt-2 text-balance text-4xl font-semibold text-ink-50">
          Choose how you want to experience Shard V1.
        </h1>
        <p className="mt-4 max-w-3xl text-sm leading-6 text-ink-300 sm:text-base">
          Start with the provenance demo if you want to understand the product quickly. Use the
          node path when you want your own capacity in the routing mix, and use the API path when
          you are integrating Shard into agent workflows.
        </p>
        {contributionStatus ? (
          <div className="mt-5 rounded-2xl border border-ring bg-base-950/40 p-4">
            <p className="text-xs uppercase tracking-[0.18em] text-accent-300">
              Local capacity status
            </p>
            <p className="mt-2 text-lg font-semibold text-ink-50">{statusText}</p>
            <p className="mt-1 text-sm text-ink-300">{contributionStatus.reason}</p>
          </div>
        ) : null}
      </section>

      <section className="mt-8 grid gap-4 lg:grid-cols-3">
        {paths.map((path, index) => (
          <article id={path.id} key={path.id} className="rounded-[1.6rem] border border-ring bg-base-900/90 p-5 shadow-panel">
            <div className="flex items-center justify-between gap-3">
              <span className="text-xs uppercase tracking-[0.18em] text-accent-300">Option {index + 1}</span>
              <a
                href={path.href}
                target={path.href.startsWith("http") ? "_blank" : undefined}
                rel={path.href.startsWith("http") ? "noreferrer" : undefined}
                className="rounded-full border border-ring px-3 py-1 text-[11px] uppercase tracking-[0.14em] text-ink-300 hover:border-accent-300 hover:text-accent-100"
              >
                {path.action}
              </a>
            </div>
            <h2 className="mt-4 text-2xl font-semibold text-ink-50">{path.title}</h2>
            <p className="mt-2 text-sm leading-6 text-ink-300">{path.summary}</p>
            <ol className="mt-5 space-y-3 text-sm text-ink-100">
              {path.steps.map((step, stepIndex) => (
                <li key={step} className="flex gap-3">
                  <span className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-accent-400/15 text-xs font-semibold text-accent-100">
                    {stepIndex + 1}
                  </span>
                  <span>{step}</span>
                </li>
              ))}
            </ol>
            <div className="mt-5 rounded-2xl border border-ring bg-base-950/40 p-4">
              <p className="text-xs uppercase tracking-[0.16em] text-ink-400">Good to know</p>
              <div className="mt-3 space-y-2 text-sm text-ink-300">
                {path.notes.map((note) => (
                  <p key={note}>{note}</p>
                ))}
              </div>
            </div>
          </article>
        ))}
      </section>

      <section className="mt-8 rounded-[1.6rem] border border-ring bg-base-900/90 p-6 shadow-panel">
        <p className="text-xs uppercase tracking-[0.22em] text-ink-400">Simple rule of thumb</p>
        <div className="mt-3 grid gap-4 md:grid-cols-3">
          <div>
            <h3 className="text-lg font-semibold text-ink-50">Need the clearest demo?</h3>
            <p className="mt-2 text-sm text-ink-300">
              Start with provenance. It shows the output, the receipt chain, and the routing graph
              together.
            </p>
          </div>
          <div>
            <h3 className="text-lg font-semibold text-ink-50">Want your own capacity?</h3>
            <p className="mt-2 text-sm text-ink-300">
              Run Shard GUI on a PC that can stay online as personal or private execution capacity.
            </p>
          </div>
          <div>
            <h3 className="text-lg font-semibold text-ink-50">Building software?</h3>
            <p className="mt-2 text-sm text-ink-300">
              Use the task APIs for workflow observability and keep chat for compatibility.
            </p>
          </div>
        </div>
        <div className="mt-6 flex flex-wrap gap-3">
          <Link href="/" className="inline-flex min-h-11 items-center justify-center rounded-xl border border-ring px-4 py-2.5 text-sm font-semibold text-ink-100 hover:bg-base-800">
            Back to overview
          </Link>
          <Link href="/provenance" className="inline-flex min-h-11 items-center justify-center rounded-xl bg-accent-500 px-4 py-2.5 text-sm font-semibold text-base-950 hover:bg-accent-400">
            Run provenance demo
          </Link>
        </div>
      </section>
    </main>
  )
}
