"use client"

import Link from "next/link"
import { useDashboardTelemetry } from "@/hooks/useDashboardTelemetry"
import { useSwarmTelemetry } from "@/hooks/useSwarmTelemetry"

const steps = [
  {
    label: "Step 1",
    title: "Give Shard one real job",
    body: "Start with the `research_brief` demo. Add a question, paste a few source notes, and set simple rules for cost, trust, and where the work is allowed to run.",
  },
  {
    label: "Step 2",
    title: "Shard chooses the best place for each step",
    body: "Planning, source summaries, and synthesis can run on your own machine, your team machines, or public specialist capacity depending on the policy.",
  },
  {
    label: "Step 3",
    title: "See the answer and the evidence",
    body: "Shard returns the final result, the receipt chain, and a provenance graph so you can see what happened instead of guessing.",
  },
] as const

const reasons = [
  {
    title: "Most AI tools hide the route",
    body: "They give you an answer but not the path. When a workflow becomes slow, expensive, or unreliable, your team is left guessing.",
  },
  {
    title: "Shard makes the route part of the product",
    body: "It shows why a step stayed local, why it moved to a private node, why it reached public capacity, and what fallback happened if the first choice failed.",
  },
  {
    title: "That matters for real teams",
    body: "Engineers can debug workflows faster. Operators can understand cost. Leaders can trust that the system is doing what policy says it should do.",
  },
] as const

const tiers = [
  {
    title: "Personal",
    body: "Your own laptop or workstation. Best when you want low-latency local work and direct control.",
  },
  {
    title: "Private",
    body: "Your company or team-owned Shard machines. Best when you want shared internal capacity without using the public market first.",
  },
  {
    title: "Public",
    body: "Specialist capacity from the wider Shard mesh. Best when you need overflow capacity or a stronger synthesis worker.",
  },
] as const

const nextMoves = [
  {
    title: "See the flagship demo",
    body: "Open the provenance page and run the `research_brief` workflow.",
    href: "/provenance",
    cta: "Open provenance demo",
  },
  {
    title: "Bring your own machine",
    body: "Run Shard on your own PC so your workflows can use your capacity first.",
    href: "/start#desktop",
    cta: "Open quick start",
  },
  {
    title: "Try simple chat",
    body: "Use chat when you only need a familiar interface. Use workflows when you need routing evidence.",
    href: "/chat",
    cta: "Open simple chat",
  },
] as const

const plainLanguage = [
  { label: "You ask", value: "One research question" },
  { label: "Shard decides", value: "Which machine should handle each step" },
  { label: "You get back", value: "An answer plus a step-by-step map" },
] as const

function formatCompact(value: number) {
  return value.toLocaleString(undefined, {
    maximumFractionDigits: value >= 100 ? 0 : 1,
  })
}

export default function HomePage() {
  const dashboard = useDashboardTelemetry()
  const { telemetry, errorMessage } = useSwarmTelemetry()
  const liveContributorCount = telemetry.contributors.length
  const contributorValue = liveContributorCount > 0 ? formatCompact(liveContributorCount) : "Waiting"
  const contributorDetail =
    liveContributorCount > 0
      ? "Machines currently reporting telemetry"
      : "Connect a Shard node to see live supply here."
  const tokenValue =
    dashboard.totalTokensGenerated > 0 ? formatCompact(dashboard.totalTokensGenerated) : "Stand by"
  const tokenDetail =
    dashboard.totalTokensGenerated > 0
      ? dashboard.isLive
        ? "Live network snapshot"
        : "Last known snapshot"
      : "Traffic appears here once the daemon reports workflow activity."

  const articleLd = {
    "@context": "https://schema.org",
    "@type": "Article",
    "headline": "Receipt-first workflow observability for AI agents",
    "description": "Shard routes AI workflow steps across personal, private, and public capacity with transparent receipts and provenance.",
    "author": {
      "@type": "Person",
      "name": "Trent Pierce",
      "url": "https://shardnetwork.live/authors/trent-pierce"
    },
    "publisher": {
      "@type": "Organization",
      "name": "Shard Network",
      "logo": "https://shardnetwork.live/brand-mark.png"
    },
    "citation": [
      "https://www.nist.gov/itl/ai-risk-management-framework",
      "https://gdpr.eu/tag/gdpr/",
      "https://standards.ieee.org/ieee/7001/6966/"
    ]
  }

  const faqLd = {
    "@context": "https://schema.org",
    "@type": "FAQPage",
    "mainEntity": [
      {
        "@type": "Question",
        "name": "What is AI workflow routing?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "AI workflow routing directs each step of an AI pipeline to the most appropriate compute capacity (personal, private, public) based on policy, cost, and trust requirements."
        }
      },
      {
        "@type": "Question",
        "name": "How does Shard provide provenance?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "Shard records durable receipts for every execution step and reconstructs a provenance graph from linked receipt IDs, providing a transparent audit trail of where and why each step ran."
        }
      },
      {
        "@type": "Question",
        "name": "What is receipt-first execution?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "Receipt-first execution means every workflow step emits a verifiable receipt containing routing details, trust tier, cost, and latency, ensuring observability is built into the runtime itself."
        }
      }
    ]
  }

  return (
    <main id="main-content" className="pb-16 pt-8 sm:pt-12">
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(articleLd) }}
      />
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(faqLd) }}
      />
      <section className="glass-panel relative overflow-hidden rounded-[2.4rem] px-6 py-8 sm:px-10 sm:py-12">
        <div className="halo-orb right-[-6rem] top-[-4rem] h-40 w-40 bg-accent-400/20" />
        <div className="halo-orb bottom-[-5rem] left-[-4rem] h-36 w-36 bg-orange-300/20" />
        <div className="grid gap-10 lg:grid-cols-[1.1fr_0.9fr] lg:items-end">
          <div>
            <div className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-4 py-2 text-[11px] font-semibold uppercase tracking-[0.28em] text-accent-300">
              <span>Shard V1</span>
              <span className="text-ink-400">Receipt-first workflow observability</span>
            </div>
            <h1 className="mt-6 max-w-4xl text-balance text-4xl font-semibold leading-tight text-ink-50 sm:text-6xl">
              The AI runtime that explains why each step ran where it did.
            </h1>
            <p className="mt-5 max-w-3xl text-base leading-8 text-ink-200 sm:text-lg">
              Shard routes AI workflow steps across personal, private, and public capacity. Then it
              gives you the answer, the receipts, and the provenance graph so you can see what
              really happened.
            </p>
            <div className="mt-6 grid gap-3 sm:grid-cols-3">
              {plainLanguage.map((item) => (
                <div key={item.label} className="data-pill rounded-[1.3rem] px-4 py-3">
                  <p className="text-[11px] uppercase tracking-[0.18em] text-ink-400">{item.label}</p>
                  <p className="mt-2 text-sm font-medium text-ink-50">{item.value}</p>
                </div>
              ))}
            </div>
            <div className="mt-8 flex flex-col gap-3 sm:flex-row">
              <Link
                href="/provenance"
                className="inline-flex min-h-11 items-center justify-center rounded-full bg-accent-500 px-6 py-3 text-sm font-semibold text-base-950 hover:bg-accent-400"
              >
                Run the demo
              </Link>
              <Link
                href="/start"
                className="inline-flex min-h-11 items-center justify-center rounded-full border border-white/10 bg-white/5 px-6 py-3 text-sm font-semibold text-ink-50 hover:bg-white/10"
              >
                Quick start
              </Link>
            </div>
          </div>

          <aside className="signal-grid rounded-[2rem] border border-white/10 bg-[linear-gradient(160deg,rgba(13,27,41,0.88),rgba(8,17,27,0.92))] p-6">
            <p className="text-xs uppercase tracking-[0.24em] text-ink-400">What Shard shows you</p>
            <div className="mt-5 grid gap-3 sm:grid-cols-2 lg:grid-cols-1">
              <MetricCard
                label="Visible route"
                value="Which machine did the work"
                detail="Each step shows whether it ran on personal, private, or public capacity."
              />
              <MetricCard
                label="Step-by-step map"
                value="Receipts + provenance"
                detail="The map is rebuilt from linked receipts, not hidden server state."
              />
              <MetricCard
                label="Live contributors"
                value={contributorValue}
                detail={contributorDetail}
              />
              <MetricCard
                label="Tokens processed"
                value={tokenValue}
                detail={tokenDetail}
              />
            </div>
            {errorMessage ? (
              <p className="mt-4 text-sm text-amber-200">Telemetry note: {errorMessage}</p>
            ) : null}
          </aside>
        </div>
      </section>

      <section className="mt-12">
        <div className="max-w-3xl">
          <p className="text-xs uppercase tracking-[0.24em] text-accent-300">How it works</p>
          <h2 className="mt-3 text-3xl font-semibold text-ink-50 sm:text-4xl">
            The full idea fits in one loop.
          </h2>
          <p className="mt-3 text-sm leading-7 text-ink-300">
            You do not need to think about distributed systems to understand the demo. Ask one
            question, add a few notes, and Shard shows you the path it took.
          </p>
        </div>
        <div className="mt-6 grid gap-4 lg:grid-cols-3">
          {steps.map((step) => (
            <article key={step.title} className="aurora-card rounded-[1.6rem] p-5">
              <p className="text-xs uppercase tracking-[0.18em] text-ink-400">{step.label}</p>
              <h3 className="mt-3 text-2xl font-semibold text-ink-50">{step.title}</h3>
              <p className="mt-3 text-sm leading-7 text-ink-200">{step.body}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="mt-12 grid gap-6 lg:grid-cols-[1.02fr_0.98fr]">
        <article className="glass-panel rounded-[1.8rem] p-6">
          <p className="text-xs uppercase tracking-[0.24em] text-accent-300">Why it feels different</p>
          <h2 className="mt-3 text-3xl font-semibold text-ink-50">
            Shard does not stop at the answer.
          </h2>
          <p className="mt-3 text-sm leading-7 text-ink-300">
            Most tools stop once the text is generated. Shard treats the route as part of the
            product, so the path is visible too.
          </p>
          <div className="mt-5 space-y-4">
            {reasons.map((reason) => (
              <div key={reason.title} className="rounded-[1.4rem] border border-white/10 bg-[rgba(255,255,255,0.03)] p-4">
                <p className="text-lg font-semibold text-ink-50">{reason.title}</p>
                <p className="mt-2 text-sm leading-7 text-ink-300">{reason.body}</p>
              </div>
            ))}
          </div>
        </article>

        <article className="glass-panel rounded-[1.8rem] p-6">
          <p className="text-xs uppercase tracking-[0.24em] text-accent-300">Three supply tiers</p>
          <h2 className="mt-3 text-3xl font-semibold text-ink-50">
            One workflow can use three kinds of capacity.
          </h2>
          <p className="mt-3 text-sm leading-7 text-ink-300">
            The same workflow can stay on your machine, move to company hardware, or reach the
            public market only when your rules allow it.
          </p>
          <div className="mt-5 space-y-4">
            {tiers.map((tier) => (
              <div key={tier.title} className="sunrise-chip rounded-[1.4rem] border border-white/10 p-4">
                <p className="text-lg font-semibold text-ink-50">{tier.title}</p>
                <p className="mt-2 text-sm leading-7 text-ink-200">{tier.body}</p>
              </div>
            ))}
          </div>
          <div className="mt-6 rounded-[1.4rem] border border-white/10 bg-[rgba(255,255,255,0.03)] p-4">
            <p className="text-sm font-semibold text-ink-50">The key idea</p>
            <p className="mt-2 text-sm leading-7 text-ink-300">
              Shard is most exciting when the routing policy and the evidence stay attached to the
              workflow. That is what makes the system debuggable instead of mysterious.
            </p>
          </div>
        </article>
      </section>

      <section className="mt-20 border-t border-white/10 pt-16">
        <div className="max-w-3xl">
          <p className="text-xs uppercase tracking-[0.24em] text-accent-300">Authoritative References</p>
          <h2 className="mt-3 text-3xl font-semibold text-ink-50 sm:text-4xl">
            Grounded in standards.
          </h2>
          <p className="mt-3 text-sm leading-7 text-ink-300">
            Shard's approach to observability and verifiable execution is informed by industry
            standards and academic research into trustworthy AI systems.
          </p>
        </div>
        <div className="mt-8 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
          <Citation 
            title="NIST AI Risk Management Framework (AI RMF 1.0)"
            source="NIST.gov"
            href="https://www.nist.gov/itl/ai-risk-management-framework"
            description="Guidelines for managing risks to individuals, organizations, and society during the design, development, use, and evaluation of AI systems."
          />
          <Citation 
            title="General Data Protection Regulation (GDPR)"
            source="GDPR.eu"
            href="https://gdpr.eu/tag/gdpr/"
            description="The toughest privacy and security law in the world, requiring organizations to be transparent about how they process personal data."
          />
          <Citation 
            title="IEEE P7001: Transparency of Autonomous Systems"
            source="IEEE.org"
            href="https://standards.ieee.org/ieee/7001/6966/"
            description="Standard for the transparency of autonomous systems, ensuring they are understandable to users and stakeholders."
          />
        </div>
      </section>
    </main>
  )
}

function Citation({ title, source, href, description }: { title: string, source: string, href: string, description: string }) {
  return (
    <div className="rounded-[1.6rem] border border-white/10 bg-white/5 p-6 space-y-3 hover:bg-white/10 transition-colors">
      <div className="flex items-center justify-between">
        <span className="text-[10px] uppercase tracking-widest text-accent-400 font-bold">{source}</span>
        <a href={href} target="_blank" rel="noreferrer" className="text-ink-400 hover:text-accent-300">
          <svg className="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="Step 10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
          </svg>
        </a>
      </div>
      <h3 className="text-lg font-semibold text-ink-50 leading-tight">{title}</h3>
      <p className="text-sm text-ink-300 leading-relaxed">{description}</p>
    </div>
  )
}

function MetricCard({
...
  detail: string
}) {
  return (
    <div className="rounded-[1.3rem] border border-white/10 bg-[rgba(255,255,255,0.03)] p-4">
      <p className="text-xs uppercase tracking-[0.18em] text-ink-400">{label}</p>
      <p className="mt-2 text-xl font-semibold text-ink-50">{value}</p>
      <p className="mt-2 text-sm text-ink-300">{detail}</p>
    </div>
  )
}
