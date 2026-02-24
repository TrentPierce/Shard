"use client"

import { useState } from "react"
import { useAppContext } from "@/lib/context"

type Section = {
  id: "verifier" | "scout" | "api"
  title: string
  summary: string
  body: string
  code: string
  codeLabel: string
}

const sections: Section[] = [
  {
    id: "verifier",
    title: "Run a Verifier",
    summary: "Desktop App from Releases",
    body:
      "Download the Shard Desktop App for Windows or macOS. The README notes one-click setup with model download and background contribution mode.",
    codeLabel: "Open releases",
    code: "https://github.com/TrentPierce/Shard/releases/latest",
  },
  {
    id: "scout",
    title: "Become a Scout",
    summary: "Browser-based participation",
    body:
      "Open the live dashboard in latest Chrome or Edge with hardware acceleration enabled, then keep the tab open to contribute scout compute.",
    codeLabel: "Open dashboard",
    code: "https://www.shardnetwork.live/",
  },
  {
    id: "api",
    title: "Consume the API",
    summary: "Install SDK and call chat",
    body: "Install the Shard SDK from PyPI and use it as a drop-in OpenAI replacement.",
    codeLabel: "Python SDK",
    code: "pip install shard-inference\n\nfrom shard import Shard\nclient = Shard()\n\nresponse = client.chat.completions.create(\n    model=\"shard-hybrid\",\n    messages=[{\"role\": \"user\", \"content\": \"Explain the Shard network architecture.\"}]\n)\nprint(response.choices[0].message.content)",
  },
]

export default function StartPage() {
  const [active, setActive] = useState<Section["id"]>("verifier")
  const { contributionStatus } = useAppContext()

  return (
    <main id="main-content" className="py-8 sm:py-10">
      <section className="rounded-2xl border border-ring bg-base-900 p-5 sm:p-8">
        <h1 className="text-balance text-3xl font-semibold text-ink-50 sm:text-4xl">Quick Start Guide</h1>
        <p className="mt-3 max-w-2xl text-sm text-ink-300 sm:text-base">
          Steps below come directly from the project README installation path: join as verifier, join as scout, then integrate the API.
        </p>
        {contributionStatus ? (
          <div className="mt-4 rounded-xl border border-ring bg-base-800 p-4">
            <p className="text-xs uppercase tracking-[0.18em] text-accent-400">Browser Contribution Status</p>
            <p className="mt-2 text-sm font-medium text-ink-50">
              {contributionStatus.state === "contributing" ? "Contributing" : "Not Contributing"}
            </p>
            <p className="mt-1 text-sm text-ink-300">{contributionStatus.reason}</p>
          </div>
        ) : null}

        <div className="mt-6 hidden gap-2 md:flex">
          {sections.map((section) => (
            <button
              key={section.id}
              type="button"
              onClick={() => setActive(section.id)}
              className={`min-h-11 rounded-lg px-4 py-2 text-sm ${
                active === section.id
                  ? "bg-accent-500 text-base-950"
                  : "border border-ring text-ink-100 hover:bg-base-800"
              }`}
            >
              {section.title}
            </button>
          ))}
        </div>

        <div className="mt-4 hidden md:block">
          {sections
            .filter((section) => section.id === active)
            .map((section) => (
              <article key={section.id} className="rounded-xl border border-ring bg-base-800 p-5">
                <p className="text-xs uppercase tracking-[0.18em] text-accent-400">{section.summary}</p>
                <p className="mt-2 text-sm text-ink-200">{section.body}</p>
                <p className="mt-4 text-xs font-medium uppercase tracking-[0.14em] text-ink-400">{section.codeLabel}</p>
                <pre className="mt-2 overflow-x-auto rounded-lg border border-ring-soft bg-base-950 p-3 text-xs text-ink-100 sm:text-sm">
                  <code className="overflow-x-auto">{section.code}</code>
                </pre>
              </article>
            ))}
        </div>

        <div className="mt-6 space-y-3 md:hidden">
          {sections.map((section) => (
            <details key={section.id} className="rounded-xl border border-ring bg-base-800 p-4" open={section.id === "verifier"}>
              <summary className="cursor-pointer list-none text-base font-medium text-ink-50">{section.title}</summary>
              <p className="mt-2 text-sm text-ink-200">{section.body}</p>
              <pre className="mt-3 overflow-x-auto rounded-lg border border-ring-soft bg-base-950 p-3 text-xs text-ink-100">
                <code className="overflow-x-auto">{section.code}</code>
              </pre>
            </details>
          ))}
        </div>
      </section>
    </main>
  )
}
