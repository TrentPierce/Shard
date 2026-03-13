"use client"

import Image from "next/image"
import Link from "next/link"
import { usePathname } from "next/navigation"
import { useEffect, useState } from "react"

const navItems = [
  { href: "/", label: "Overview" },
  { href: "/provenance", label: "Provenance" },
  { href: "/start", label: "Quick Start" },
  { href: "/chat", label: "Simple Chat" },
]

export function SiteNav() {
  const pathname = usePathname()
  const [open, setOpen] = useState(false)

  useEffect(() => {
    setOpen(false)
  }, [pathname])

  return (
    <header className="sticky top-0 z-40 border-b border-white/10 bg-[rgba(7,19,29,0.78)] backdrop-blur-xl">
      <div className="mx-auto flex h-16 w-full max-w-7xl items-center justify-between px-4 sm:px-6 lg:px-8">
        <Link href="/" className="group flex items-center gap-3">
          <Image
            src="/brand-mark.png"
            alt=""
            width={40}
            height={40}
            className="h-10 w-10 rounded-2xl border border-white/10 bg-white/5 p-1.5 transition group-hover:border-accent-300/70"
            aria-hidden="true"
          />
          <div className="leading-tight">
            <p className="text-[11px] font-semibold uppercase tracking-[0.34em] text-accent-300">
              Shard
            </p>
            <p className="text-sm text-ink-200">See why AI work ran there</p>
          </div>
        </Link>

        <nav className="hidden items-center gap-2 md:flex" aria-label="Primary">
          {navItems.map((item) => {
            const active = pathname === item.href
            return (
              <Link
                key={item.href}
                href={item.href}
                className={`rounded-full px-4 py-2 text-sm ${
                  active
                    ? "sunrise-chip border border-white/10 text-ink-50"
                    : "text-ink-300 hover:text-ink-50"
                }`}
              >
                {item.label}
              </Link>
            )
          })}
        </nav>

        <div className="hidden items-center gap-3 md:flex">
          <a
            href="https://github.com/TrentPierce/Shard"
            target="_blank"
            rel="noreferrer"
            className="text-sm text-ink-300 hover:text-ink-50"
          >
            GitHub
          </a>
          <Link
            href="/provenance"
            className="inline-flex min-h-11 items-center justify-center rounded-full bg-accent-500 px-4 py-2 text-sm font-semibold text-base-950 hover:bg-accent-400"
          >
            Run the demo
          </Link>
        </div>

        <button
          type="button"
          className="inline-flex h-11 w-11 items-center justify-center rounded-full border border-white/10 bg-white/5 text-ink-100 md:hidden"
          aria-controls="mobile-nav"
          aria-expanded={open}
          aria-label="Toggle menu"
          onClick={() => setOpen((prev) => !prev)}
        >
          <span aria-hidden="true" className="text-lg leading-none">
            {open ? "\u2715" : "\u2630"}
          </span>
        </button>
      </div>

      {open ? (
        <nav
          id="mobile-nav"
          className="border-t border-white/10 bg-[rgba(7,19,29,0.94)] px-4 py-4 md:hidden"
          aria-label="Mobile menu"
        >
          <div className="flex flex-col gap-2">
            {navItems.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className="rounded-2xl border border-white/10 px-4 py-3 text-ink-100"
                onClick={() => setOpen(false)}
              >
                {item.label}
              </Link>
            ))}
            <Link
              href="/provenance"
              className="inline-flex min-h-11 items-center justify-center rounded-2xl bg-accent-500 px-4 py-3 text-sm font-semibold text-base-950"
              onClick={() => setOpen(false)}
            >
              Run the demo
            </Link>
          </div>
        </nav>
      ) : null}
    </header>
  )
}
