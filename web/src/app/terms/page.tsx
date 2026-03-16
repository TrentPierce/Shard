import { Metadata } from "next"

export const metadata: Metadata = {
  title: "Terms of Service | Shard Network",
  description: "Terms and conditions for using the Shard Network platform and services.",
}

export default function TermsPage() {
  return (
    <main className="mx-auto max-w-4xl px-4 py-20 sm:px-6 lg:px-8">
      <div className="space-y-8 prose prose-invert max-w-none">
        <h1 className="text-4xl font-bold tracking-tight text-ink-100 sm:text-5xl">
          Terms of Service
        </h1>
        <p className="text-xl text-ink-300">Last updated: March 16, 2026</p>

        <section className="space-y-4">
          <h2 className="text-2xl font-bold text-ink-100">1. Acceptance of Terms</h2>
          <p className="text-ink-300">
            By accessing or using Shard Network, you agree to be bound by these Terms of Service.
          </p>
        </section>

        <section className="space-y-4">
          <h2 className="text-2xl font-bold text-ink-100">2. Use of Service</h2>
          <p className="text-ink-300">
            You agree to use Shard Network only for purposes that are permitted by these Terms 
            and any applicable law, regulation, or generally accepted practices or guidelines 
            in the relevant jurisdictions.
          </p>
        </section>

        <section className="space-y-4">
          <h2 className="text-2xl font-bold text-ink-100">3. Functional Source License</h2>
          <p className="text-ink-300">
            Shard is licensed under the Functional Source License 1.1 (FSL-1.1-ALv2). 
            Please refer to our LICENSE file for more details.
          </p>
        </section>
      </div>
    </main>
  )
}
