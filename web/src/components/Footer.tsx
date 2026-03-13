"use client"

import Image from "next/image"
import Link from "next/link"
import { Github } from "lucide-react"
import { SHARD_VERSION } from "@/lib/version"

const footerSections = [
  {
    title: "Start",
    links: [
      { name: "Overview", href: "/" },
      { name: "Provenance Demo", href: "/provenance" },
      { name: "Quick Start", href: "/start" },
      { name: "Simple Chat", href: "/chat" },
    ],
  },
  {
    title: "Learn",
    links: [
      { name: "API Reference", href: "https://github.com/TrentPierce/Shard/blob/main/docs/api.md" },
      { name: "Architecture", href: "https://github.com/TrentPierce/Shard/blob/main/docs/architecture.md" },
      { name: "Run a Node", href: "https://github.com/TrentPierce/Shard/blob/main/docs/run-a-node.md" },
      { name: "Python SDK", href: "https://github.com/TrentPierce/Shard/tree/main/sdk/python" },
    ],
  },
] as const

export default function Footer() {
  const currentYear = new Date().getFullYear()

  return (
    <footer className="relative z-10 mt-20 border-t border-white/10 bg-[linear-gradient(180deg,rgba(9,17,27,0),rgba(9,17,27,0.92))]">
      <div className="mx-auto grid w-full max-w-7xl gap-10 px-4 py-10 sm:px-6 lg:grid-cols-[1.2fr_0.8fr_0.8fr] lg:px-8">
        <div className="space-y-4">
          <Link href="/" className="inline-flex items-center gap-3">
            <Image
              src="/brand-mark.png"
              alt=""
              width={40}
              height={40}
              className="h-10 w-10 rounded-2xl border border-white/10 bg-white/5 p-1.5"
              aria-hidden="true"
            />
            <div>
              <p className="text-xs uppercase tracking-[0.32em] text-accent-300">Shard</p>
              <p className="text-sm text-ink-200">Receipt-first workflow observability</p>
            </div>
          </Link>
          <p className="max-w-md text-sm leading-6 text-ink-300">
            Shard helps AI teams understand where each workflow step ran, why it was routed there,
            and what happened when the preferred path failed.
          </p>
          <div className="flex flex-wrap gap-3">
            <span className="sunrise-chip rounded-full border border-white/10 px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.18em] text-ink-100">
              Personal
            </span>
            <span className="sunrise-chip rounded-full border border-white/10 px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.18em] text-ink-100">
              Private
            </span>
            <span className="sunrise-chip rounded-full border border-white/10 px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.18em] text-ink-100">
              Public
            </span>
          </div>
        </div>

        {footerSections.map((section) => (
          <div key={section.title}>
            <h2 className="text-xs uppercase tracking-[0.24em] text-ink-400">{section.title}</h2>
            <ul className="mt-4 space-y-3 text-sm text-ink-200">
              {section.links.map((link) => {
                const external = link.href.startsWith("http")
                return (
                  <li key={link.name}>
                    <a
                      href={link.href}
                      target={external ? "_blank" : undefined}
                      rel={external ? "noreferrer" : undefined}
                      className="hover:text-accent-300"
                    >
                      {link.name}
                    </a>
                  </li>
                )
              })}
            </ul>
          </div>
        ))}
      </div>

      <div className="mx-auto flex w-full max-w-7xl flex-col gap-3 border-t border-white/10 px-4 py-4 text-sm text-ink-400 sm:flex-row sm:items-center sm:justify-between sm:px-6 lg:px-8">
        <p>
          &copy; {currentYear} Shard Network. v{SHARD_VERSION}
        </p>
        <div className="flex items-center gap-4">
          <a
            href="https://github.com/TrentPierce/Shard"
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-2 hover:text-accent-300"
          >
            <Github size={16} />
            <span>GitHub</span>
          </a>
        </div>
      </div>
    </footer>
  )
}
