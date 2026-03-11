"use client"

import Link from "next/link"
import { useMemo } from "react"
import { useAppContext } from "@/lib/context"

type PathCard = {
  id: "browser" | "desktop" | "api"
  title: string
  summary: string
  action: string
  href: string
  steps: string[]
  notes: string[]
}

const paths: PathCard[] = [
  {
    id: "browser",
    title: "Browser Client",
    summary: "The easiest way to use Shard. Simple prompts can finish locally in-browser with no install required.",
    action: "Open shardnetwork.live",
    href: "https://www.shardnetwork.live/",
    steps: [
      "Open the site in Chrome or Edge.",
      "Use Auto mode to let the browser answer lighter prompts locally.",
      "Escalate harder prompts to the desktop verifier path only when needed.",
    ],
    notes: [
      "WAN browser scouts are now experimental and are only needed for benchmark or research sessions.",
      "If WebGPU is unavailable, the page can still route requests to the network path.",
    ],
  },
  {
    id: "desktop",
    title: "Desktop Verifier",
    summary: "The best path for a spare PC that should stay online and help the mesh.",
    action: "Download Shard GUI",
    href: "https://github.com/TrentPierce/Shard/releases/latest",
    steps: [
      "Download and open Shard GUI.",
      "Let the model finish downloading on first run.",
      "Save settings, restart the node once, then click Start.",
    ],
    notes: [
      "After the model finishes downloading, restart once so the verifier comes up cleanly.",
      "If health is green and the node shows a peer connection, it has joined the mesh.",
    ],
  },
  {
    id: "api",
    title: "API Consumer",
    summary: "Use the network like an OpenAI-compatible backend in your own app.",
    action: "Read API docs",
    href: "https://github.com/TrentPierce/Shard#python-sdk",
    steps: [
      "Point your client to the Shard endpoint.",
      "Send standard chat-completions requests.",
      "Check network health before production traffic.",
    ],
    notes: [
      "This path is for developers. Most users should start with browser or desktop.",
      "The API shape is already familiar if you have used OpenAI-compatible SDKs.",
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
        <h1 className="mt-2 text-balance text-4xl font-semibold text-ink-50">Choose the easiest way for you to join the network.</h1>
        <p className="mt-4 max-w-3xl text-sm leading-6 text-ink-300 sm:text-base">
          You do not need to understand distributed systems to use Shard. Pick the browser path for local-first responses, or the desktop path if you want your computer to act as a stronger verifier node.
        </p>
        {contributionStatus ? (
          <div className="mt-5 rounded-2xl border border-ring bg-base-950/40 p-4">
            <p className="text-xs uppercase tracking-[0.18em] text-accent-300">Browser status</p>
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
            <h3 className="text-lg font-semibold text-ink-50">Just want to help?</h3>
            <p className="mt-2 text-sm text-ink-300">Use the browser client. It is the least technical path and now answers lighter prompts locally first.</p>
          </div>
          <div>
            <h3 className="text-lg font-semibold text-ink-50">Want more impact?</h3>
            <p className="mt-2 text-sm text-ink-300">Run Shard GUI on a PC that can stay online as a verifier node.</p>
          </div>
          <div>
            <h3 className="text-lg font-semibold text-ink-50">Building software?</h3>
            <p className="mt-2 text-sm text-ink-300">Use the API path and treat Shard like an OpenAI-compatible backend.</p>
          </div>
        </div>
        <div className="mt-6 flex flex-wrap gap-3">
          <Link href="/" className="inline-flex min-h-11 items-center justify-center rounded-xl border border-ring px-4 py-2.5 text-sm font-semibold text-ink-100 hover:bg-base-800">
            Back to dashboard
          </Link>
          <Link href="/chat" className="inline-flex min-h-11 items-center justify-center rounded-xl bg-accent-500 px-4 py-2.5 text-sm font-semibold text-base-950 hover:bg-accent-400">
            Test the API
          </Link>
        </div>
      </section>
    </main>
  )
}
