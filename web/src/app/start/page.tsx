"use client"

import Link from "next/link"
import { useMemo } from "react"
import { useAppContext } from "@/lib/context"

const paths = [
  {
    id: "provenance",
    title: "Start with the demo",
    summary: "Best for first-time visitors. You will see the final answer, the raw receipts, and the full provenance graph together.",
    action: "Open provenance demo",
    href: "/provenance",
    steps: [
      "Ask one research question.",
      "Paste a few source documents.",
      "Run the workflow and inspect why each step ran where it did.",
    ],
  },
  {
    id: "desktop",
    title: "Add your own machine",
    summary: "Best when you want your own PC or team hardware in the routing mix before public fallback.",
    action: "Download Shard GUI",
    href: "https://github.com/TrentPierce/Shard/releases/latest",
    steps: [
      "Download the latest Shard GUI release.",
      "Let the model finish downloading on first run.",
      "Restart once, click Start, and confirm the node is healthy.",
    ],
  },
  {
    id: "api",
    title: "Integrate the API",
    summary: "Best for developers. Keep chat for compatibility, then move to tasks when you need workflow observability.",
    action: "Read API docs",
    href: "https://github.com/TrentPierce/Shard/blob/main/docs/api.md",
    steps: [
      "Use `/v1/chat/completions` for simple compatibility work.",
      "Use `/v1/agents/tasks` for the `research_brief` workflow.",
      "Fetch receipts and provenance graphs to debug the route after the run.",
    ],
  },
]

export default function StartPage() {
  const { contributionStatus } = useAppContext()
  const statusText = useMemo(() => {
    if (!contributionStatus) return null
    if (contributionStatus.state === "contributing") return "Active"
    if (contributionStatus.state === "starting") return "Starting"
    return "Idle"
  }, [contributionStatus])

  const howToLd = {
    "@context": "https://schema.org",
    "@type": "HowTo",
    "name": "How to get started with Shard Network",
    "description": "Choose the best path to start using Shard, from running the provenance demo to adding your own compute capacity.",
    "step": [
      {
        "@type": "HowToStep",
        "name": "Run the provenance demo",
        "text": "Ask a research question, paste source documents, and inspect the resulting provenance graph.",
        "url": "https://shardnetwork.live/provenance"
      },
      {
        "@type": "HowToStep",
        "name": "Add your own machine",
        "text": "Download the Shard GUI, download models, and start your local node.",
        "url": "https://github.com/TrentPierce/Shard/releases/latest"
      },
      {
        "@type": "HowToStep",
        "name": "Integrate the API",
        "text": "Use the workflow APIs to submit tasks and retrieve receipts and provenance data.",
        "url": "https://github.com/TrentPierce/Shard/blob/main/docs/api.md"
      }
    ]
  }

  return (
    <main id="main-content" className="py-8 sm:py-10">
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(howToLd) }}
      />
      <section className="glass-panel rounded-[2rem] px-6 py-8 sm:px-8 sm:py-10">
        <p className="text-xs uppercase tracking-[0.24em] text-accent-300">Quick start</p>
        <h1 className="mt-3 max-w-3xl text-balance text-4xl font-semibold text-ink-50 sm:text-5xl">
          Pick the easiest path into Shard.
        </h1>
        <p className="mt-4 max-w-3xl text-base leading-7 text-ink-200">
          If you only do one thing, run the provenance demo first. It shows what Shard actually
          does: route one AI workflow across personal, private, and public capacity while explaining
          every decision.
        </p>
        <div className="mt-5 grid gap-3 sm:grid-cols-3">
          <Callout label="What you bring" value="A question and a few source notes" />
          <Callout label="What Shard does" value="Chooses the best place for each step" />
          <Callout label="What you can inspect" value="Answer, receipts, and the map" />
        </div>
        <div className="mt-6 grid gap-4 md:grid-cols-3">
          <Callout label="Fastest to understand" value="Provenance demo" />
          <Callout label="Best for ownership" value="Run your own node" />
          <Callout label="Best for builders" value="Use the API + SDK" />
        </div>
        {contributionStatus ? (
          <div className="mt-6 rounded-[1.5rem] border border-white/10 bg-[rgba(255,255,255,0.03)] p-4">
            <p className="text-xs uppercase tracking-[0.18em] text-ink-400">Local capacity status</p>
            <p className="mt-2 text-lg font-semibold text-ink-50">{statusText}</p>
            <p className="mt-1 text-sm text-ink-300">{contributionStatus.reason}</p>
          </div>
        ) : null}
      </section>

      <section className="mt-8 grid gap-4 lg:grid-cols-3">
        {paths.map((path, index) => {
          const external = path.href.startsWith("http")
          return (
            <article id={path.id} key={path.id} className="glass-panel rounded-[1.6rem] p-5">
              <div className="flex items-center justify-between gap-3">
                <span className="text-xs uppercase tracking-[0.18em] text-ink-400">
                  Option {index + 1}
                </span>
                <a
                  href={path.href}
                  target={external ? "_blank" : undefined}
                  rel={external ? "noreferrer" : undefined}
                  className="rounded-full border border-white/10 px-3 py-1 text-[11px] uppercase tracking-[0.14em] text-accent-300 hover:border-accent-300 hover:text-ink-50"
                >
                  {path.action}
                </a>
              </div>
              <h2 className="mt-4 text-2xl font-semibold text-ink-50">{path.title}</h2>
              <p className="mt-3 text-sm leading-6 text-ink-300">{path.summary}</p>
              <ol className="mt-5 space-y-3 text-sm text-ink-100">
                {path.steps.map((step, stepIndex) => (
                  <li key={step} className="flex gap-3">
                    <span className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-accent-500/16 text-xs font-semibold text-accent-300">
                      {stepIndex + 1}
                    </span>
                    <span>{step}</span>
                  </li>
                ))}
              </ol>
            </article>
          )
        })}
      </section>

      <section className="mt-8 grid gap-4 lg:grid-cols-[1.05fr_0.95fr]">
        <article className="glass-panel rounded-[1.6rem] p-6">
          <p className="text-xs uppercase tracking-[0.24em] text-ink-400">Simple rule</p>
          <h2 className="mt-2 text-3xl font-semibold text-ink-50">
            Start with the proof, then add the rest.
          </h2>
          <div className="mt-5 space-y-4">
            <FactRow
              title="New to Shard?"
              body="Open the provenance demo first. It is the clearest explanation of the product."
            />
            <FactRow
              title="Need your own hardware involved?"
              body="Run a node so Shard can prefer your own machine or your team’s machines."
            />
            <FactRow
              title="Building software?"
              body="Keep chat for compatibility and use the workflow APIs when you need routing evidence."
            />
          </div>
        </article>

        <article className="glass-panel rounded-[1.6rem] p-6">
          <p className="text-xs uppercase tracking-[0.24em] text-ink-400">The shortest explanation</p>
          <h2 className="mt-2 text-3xl font-semibold text-ink-50">
            Shard helps you answer one question:
          </h2>
          <p className="mt-4 text-xl font-semibold text-ink-50">
            “Why did this AI step run there?”
          </p>
          <p className="mt-4 text-sm leading-7 text-ink-200">
            The Shard demo answers that question with receipts, provenance, and a final result you
            can actually inspect.
          </p>
          <div className="mt-6 flex flex-wrap gap-3">
            <Link
              href="/provenance"
              className="inline-flex min-h-11 items-center justify-center rounded-full bg-accent-500 px-5 py-3 text-sm font-semibold text-base-950 hover:bg-accent-400"
            >
              Run the demo
            </Link>
            <Link
              href="/"
              className="inline-flex min-h-11 items-center justify-center rounded-full border border-white/10 bg-white/5 px-5 py-3 text-sm font-semibold text-ink-50 hover:bg-white/10"
            >
              Back to overview
            </Link>
          </div>
        </article>
      </section>
    </main>
  )
}

function Callout({ label, value }: { label: string; value: string }) {
  return (
    <div className="sunrise-chip rounded-[1.5rem] border border-white/10 p-4">
      <p className="text-xs uppercase tracking-[0.18em] text-ink-400">{label}</p>
      <p className="mt-2 text-xl font-semibold text-ink-50">{value}</p>
    </div>
  )
}

function FactRow({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-2xl border border-white/10 bg-[rgba(255,255,255,0.03)] p-4">
      <p className="text-lg font-semibold text-ink-50">{title}</p>
      <p className="mt-2 text-sm leading-6 text-ink-300">{body}</p>
    </div>
  )
}
