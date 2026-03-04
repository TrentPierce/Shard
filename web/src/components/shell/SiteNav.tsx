"use client"

import Link from "next/link"
import { usePathname } from "next/navigation"
import { useEffect, useState } from "react"

const navItems = [
  { href: "/", label: "Dashboard" },
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
    <header className="sticky top-0 z-40 border-b border-ring bg-base-900/90 backdrop-blur">
      <div className="mx-auto flex h-16 w-full max-w-7xl items-center justify-between px-4 sm:px-6 lg:px-8">
        <Link href="/" className="flex items-center gap-2.5 text-sm font-semibold tracking-wide text-ink-50">
          <img src="/icon-192.png" alt="" className="h-7 w-7 rounded-lg" aria-hidden="true" />
          Shard
        </Link>

        <nav className="hidden items-center gap-2 md:flex" aria-label="Primary">
          {navItems.map((item) => (
            <Link
              key={item.href}
              href={item.href}
              className={`min-h-11 min-w-11 rounded-lg px-4 py-2 text-sm transition ${
                pathname === item.href
                  ? "bg-base-800 text-ink-50"
                  : "text-ink-300 hover:bg-base-800 hover:text-ink-50"
              }`}
            >
              {item.label}
            </Link>
          ))}
          <a
            href="https://github.com/TrentPierce/Shard"
            target="_blank"
            rel="noreferrer"
            className="min-h-11 min-w-11 rounded-lg px-4 py-2 text-sm text-ink-300 transition hover:bg-base-800 hover:text-ink-50"
          >
            GitHub
          </a>
        </nav>

        <button
          type="button"
          className="inline-flex h-11 w-11 items-center justify-center rounded-lg border border-ring text-ink-100 md:hidden"
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
        <nav id="mobile-nav" className="border-t border-ring px-4 py-3 md:hidden" aria-label="Mobile menu">
          <div className="flex flex-col gap-2">
            {navItems.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className="min-h-11 rounded-lg px-3 py-2 text-ink-100 hover:bg-base-800"
                onClick={() => setOpen(false)}
              >
                {item.label}
              </Link>
            ))}
            <a
              href="https://github.com/TrentPierce/Shard"
              target="_blank"
              rel="noreferrer"
              className="min-h-11 rounded-lg px-3 py-2 text-ink-100 hover:bg-base-800"
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
