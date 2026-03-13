"use client"

import Link from "next/link"
import Image from "next/image"
import { usePathname } from "next/navigation"
import { useEffect, useState } from "react"

const navItems = [
  { href: "/", label: "Overview" },
  { href: "/provenance", label: "Provenance" },
  { href: "/chat", label: "Chat" },
  { href: "/start", label: "Quick Start" },
]

export function SiteNav() {
  const pathname = usePathname()
  const [open, setOpen] = useState(false)

  useEffect(() => {
    setOpen(false)
  }, [pathname])

  return (
    <header className="sticky top-0 z-40 border-b border-ring/70 bg-base-950/95 backdrop-blur">
      <div className="mx-auto flex h-16 w-full max-w-7xl items-center justify-between px-4 sm:px-6 lg:px-8">
        <Link href="/" className="group flex items-center gap-3 text-sm font-semibold tracking-[0.18em] text-ink-50">
          <Image
            src="/brand-mark.png"
            alt=""
            width={36}
            height={36}
            className="h-9 w-9 rounded-xl border border-white/12 bg-white/5 p-1 transition group-hover:border-accent-300/60"
            aria-hidden="true"
          />
          <span className="flex flex-col">
            <span>Shard</span>
            <span className="text-[10px] font-medium tracking-[0.28em] text-ink-300">
              RECEIPT FIRST
            </span>
          </span>
        </Link>

        <nav className="hidden items-center gap-2 md:flex" aria-label="Primary">
          {navItems.map((item) => (
            <Link
              key={item.href}
              href={item.href}
              className={`min-h-11 min-w-11 rounded-lg px-4 py-2 text-sm transition ${
                pathname === item.href
                  ? "bg-accent-500/18 text-ink-50"
                  : "text-ink-300 hover:bg-white/6 hover:text-ink-50"
              }`}
            >
              {item.label}
            </Link>
          ))}
          <a
            href="https://github.com/TrentPierce/Shard"
            target="_blank"
            rel="noreferrer"
            className="min-h-11 min-w-11 rounded-lg px-4 py-2 text-sm text-ink-300 transition hover:bg-white/6 hover:text-ink-50"
          >
            GitHub
          </a>
        </nav>

        <button
          type="button"
          className="inline-flex h-11 w-11 items-center justify-center rounded-lg border border-ring bg-white/5 text-ink-100 md:hidden"
          aria-controls="mobile-nav"
          aria-expanded={open}
          aria-label="Toggle menu"
          onClick={() => setOpen((prev) => !prev)}
        >
          <span className="sr-only">Menu</span>
          <span aria-hidden="true" className="text-lg leading-none">{open ? "\u2715" : "\u2630"}</span>
        </button>
      </div>

      {open ? (
        <nav id="mobile-nav" className="border-t border-ring/70 bg-base-950/70 px-4 py-3 md:hidden" aria-label="Mobile menu">
          <div className="flex flex-col gap-2">
            {navItems.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className="min-h-11 rounded-lg px-3 py-2 text-ink-100 hover:bg-white/6"
                onClick={() => setOpen(false)}
              >
                {item.label}
              </Link>
            ))}
            <a
              href="https://github.com/TrentPierce/Shard"
              target="_blank"
              rel="noreferrer"
              className="min-h-11 rounded-lg px-3 py-2 text-ink-100 hover:bg-white/6"
              onClick={() => setOpen(false)}
            >
              GitHub
            </a>
          </div>
        </nav>
      ) : null}
    </header>
  )
}
