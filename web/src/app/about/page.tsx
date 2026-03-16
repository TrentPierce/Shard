import { Metadata } from "next"
import Image from "next/image"
import Link from "next/link"
import { authors } from "@/lib/authors"

export const metadata: Metadata = {
  title: "About Shard | Our Mission and Team",
  description: "Learn about Shard's mission to make AI agent workflows transparent, verifiable, and observable through receipt-first execution.",
}

export default function AboutPage() {
  const jsonLd = {
    "@context": "https://schema.org",
    "@type": "Organization",
    "name": "Shard Network",
    "url": "https://shardnetwork.live",
    "logo": "https://shardnetwork.live/brand-mark.png",
    "description": "Shard helps AI teams see why each workflow step ran where it did with receipts, provenance graphs, and policy-aware routing.",
    "foundingDate": "2024",
    "sameAs": [
      "https://github.com/TrentPierce/Shard",
      "https://linkedin.com/in/trentpierce"
    ],
    "contactPoint": [
      {
        "@type": "ContactPoint",
        "email": "hello@shardnetwork.live",
        "contactType": "customer service"
      }
    ],
    "founders": authors.map(a => ({
      "@type": "Person",
      "name": a.name,
      "jobTitle": a.role,
      "url": `https://shardnetwork.live/authors/${a.slug}`
    }))
  }

  return (
    <main className="mx-auto max-w-4xl px-4 py-20 sm:px-6 lg:px-8">
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
      />

      <div className="space-y-16">
        <section className="space-y-6">
          <h1 className="text-4xl font-bold tracking-tight text-ink-100 sm:text-5xl">
            Our Mission
          </h1>
          <p className="text-xl leading-relaxed text-ink-300">
            Shard's mission is to make multi-step agent workflows understandable instead of opaque. 
            We believe that as AI agents take on more critical tasks, the ability to verify where, 
            why, and how a task was executed becomes a fundamental requirement, not an optional feature.
          </p>
        </section>

        <section className="space-y-6 text-ink-300">
          <h2 className="text-3xl font-bold tracking-tight text-ink-100">The Problem</h2>
          <p className="text-lg leading-relaxed">
            Most AI platforms can tell you the answer. Very few can tell you, in plain terms, 
            why a task used your own machine instead of the public market, why a specific 
            specialist was chosen for synthesis, or what fallback fired when a node failed.
          </p>
        </section>

        <section className="space-y-6">
          <h2 className="text-3xl font-bold tracking-tight text-ink-100 text-center md:text-left">Our Team</h2>
          <div className="grid gap-8 sm:grid-cols-2">
            {authors.map((author) => (
              <Link 
                key={author.slug} 
                href={`/authors/${author.slug}`}
                className="group relative flex flex-col items-center gap-4 rounded-2xl border border-white/10 bg-white/5 p-6 transition-colors hover:bg-white/10"
              >
                <div className="relative h-32 w-32 overflow-hidden rounded-xl border border-white/10">
                  <Image
                    src={author.photo}
                    alt={author.name}
                    fill
                    className="object-cover transition-transform group-hover:scale-110"
                  />
                </div>
                <div className="text-center">
                  <h3 className="text-xl font-bold text-ink-100">{author.name}</h3>
                  <p className="text-accent-300">{author.role}</p>
                </div>
              </Link>
            ))}
          </div>
        </section>

        <section className="rounded-2xl border border-white/10 bg-white/5 p-8 space-y-4">
          <h2 className="text-2xl font-bold text-ink-100">History</h2>
          <p className="text-ink-300">
            Shard V1 was built with a "receipt-first" philosophy. We realized that by building 
            execution runtimes that emit durable receipts at every step, we could reconstruct 
            the entire history of a workflow without relying on a centralized coordinator's state.
          </p>
        </section>
      </div>
    </main>
  )
}
