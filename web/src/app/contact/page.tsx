import { Metadata } from "next"
import { Mail, Github, MessageSquare } from "lucide-react"

export const metadata: Metadata = {
  title: "Contact Us | Shard Network",
  description: "Get in touch with the Shard team for support, partnership inquiries, or to report issues.",
}

export default function ContactPage() {
  const jsonLd = {
    "@context": "https://schema.org",
    "@type": "Organization",
    url: "https://shardnetwork.live",
    name: "Shard Network",
    contactPoint: [
      {
        "@type": "ContactPoint",
        "email": "hello@shardnetwork.live",
        "contactType": "customer support"
      }
    ]
  }

  return (
    <main className="mx-auto max-w-4xl px-4 py-20 sm:px-6 lg:px-8">
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
      />

      <div className="space-y-12">
        <div className="space-y-4">
          <h1 className="text-4xl font-bold tracking-tight text-ink-100 sm:text-5xl">
            Contact Us
          </h1>
          <p className="text-xl text-ink-300">
            Have questions about Shard? We're here to help.
          </p>
        </div>

        <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
          <a
            href="mailto:hello@shardnetwork.live"
            className="flex flex-col items-start gap-4 rounded-2xl border border-white/10 bg-white/5 p-8 transition-colors hover:bg-white/10"
          >
            <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-accent-500/10 text-accent-300">
              <Mail size={24} />
            </div>
            <div>
              <h3 className="text-lg font-bold text-ink-100">Email</h3>
              <p className="text-sm text-ink-300">hello@shardnetwork.live</p>
            </div>
          </a>

          <a
            href="https://github.com/TrentPierce/Shard/issues"
            target="_blank"
            rel="noreferrer"
            className="flex flex-col items-start gap-4 rounded-2xl border border-white/10 bg-white/5 p-8 transition-colors hover:bg-white/10"
          >
            <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-accent-500/10 text-accent-300">
              <Github size={24} />
            </div>
            <div>
              <h3 className="text-lg font-bold text-ink-100">GitHub Issues</h3>
              <p className="text-sm text-ink-300">Report bugs or request features</p>
            </div>
          </a>

          <a
            href="https://github.com/TrentPierce/Shard/discussions"
            target="_blank"
            rel="noreferrer"
            className="flex flex-col items-start gap-4 rounded-2xl border border-white/10 bg-white/5 p-8 transition-colors hover:bg-white/10"
          >
            <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-accent-500/10 text-accent-300">
              <MessageSquare size={24} />
            </div>
            <div>
              <h3 className="text-lg font-bold text-ink-100">Discussions</h3>
              <p className="text-sm text-ink-300">Join our community</p>
            </div>
          </a>
        </div>

        <div className="rounded-2xl border border-white/10 bg-white/5 p-8">
          <h2 className="text-2xl font-bold text-ink-100">Company Address</h2>
          <p className="mt-2 text-ink-300">
            Shard Network Labs<br />
            San Francisco, CA<br />
            United States
          </p>
        </div>
      </div>
    </main>
  )
}
