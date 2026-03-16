import { Metadata } from "next"

export const metadata: Metadata = {
  title: "Privacy Policy | Shard Network",
  description: "Our privacy policy describes how we collect, use, and handle your information when you use Shard Network.",
}

export default function PrivacyPage() {
  return (
    <main className="mx-auto max-w-4xl px-4 py-20 sm:px-6 lg:px-8">
      <div className="space-y-8 prose prose-invert max-w-none">
        <h1 className="text-4xl font-bold tracking-tight text-ink-100 sm:text-5xl">
          Privacy Policy
        </h1>
        <p className="text-xl text-ink-300">Last updated: March 16, 2026</p>

        <section className="space-y-4">
          <h2 className="text-2xl font-bold text-ink-100">1. Introduction</h2>
          <p className="text-ink-300">
            Shard Network ("we", "us", or "our") is committed to protecting your privacy. This Privacy Policy 
            explains how your personal information is collected, used, and disclosed by Shard Network.
          </p>
        </section>

        <section className="space-y-4">
          <h2 className="text-2xl font-bold text-ink-100">2. Information Collection</h2>
          <p className="text-ink-300">
            We collect information that you provide directly to us when you use our website, SDK, 
            or interact with us. This may include your name, email address, and any other information 
            you choose to provide.
          </p>
        </section>

        <section className="space-y-4">
          <h2 className="text-2xl font-bold text-ink-100">3. GDPR Compliance</h2>
          <p className="text-ink-300">
            For users in the European Economic Area (EEA), we process your personal data in accordance 
            with the General Data Protection Regulation (GDPR). You have the right to access, 
            rectify, or erase your personal data.
          </p>
        </section>

        <section className="space-y-4">
          <h2 className="text-2xl font-bold text-ink-100">4. Contact Us</h2>
          <p className="text-ink-300">
            If you have any questions about this Privacy Policy, please contact us at hello@shardnetwork.live.
          </p>
        </section>
      </div>
    </main>
  )
}
