"use client"

import { useEffect, useState } from "react"
import { apiUrl } from "@/lib/config"

export default function LeaderboardPage() {
  const [data, setData] = useState<any[]>([])

  useEffect(() => {
    fetch(apiUrl("/v1/leaderboard"))
      .then((r) => r.json())
      .then((d) => {
        if (d.ok && d.leaderboard) setData(d.leaderboard)
      })
      .catch(() => {
        // ignore fetch failures and show empty state
      })
  }, [])

  return (
    <main id="main-content" className="mb-20 mt-8 sm:mt-12">
      <section className="glass-panel rounded-[2rem] px-6 py-8 sm:px-8 sm:py-10">
        <p className="text-xs uppercase tracking-[0.24em] text-accent-300">Operator ledger</p>
        <h1 className="mt-3 text-4xl font-semibold text-ink-50 sm:text-5xl">Contributor balances</h1>
        <p className="mt-4 max-w-3xl text-sm leading-7 text-ink-200">
          This page is for operators tracking the legacy credit surface. The main Shard product is
          now the receipt-first provenance flow, so most visitors should start with the Provenance
          demo instead.
        </p>
        <div className="mt-6 rounded-2xl border border-amber-300/20 bg-amber-300/10 p-4 text-sm text-amber-100">
          If you are new here, open the Provenance page first. This ledger remains available for
          network operators and historical balance tracking.
        </div>
      </section>

      <div className="mt-8 overflow-hidden rounded-2xl border border-ring bg-base-900">
        <table className="w-full border-collapse text-left">
          <thead className="border-b border-ring bg-base-800/40 text-sm text-ink-200">
            <tr>
              <th className="w-24 p-4 font-semibold">Rank</th>
              <th className="p-4 font-semibold">Wallet</th>
              <th className="p-4 text-right font-semibold">Legacy Credit Balance</th>
            </tr>
          </thead>
          <tbody>
            {data.map((row, i) => (
              <tr key={row.wallet} className="border-b border-ring/35 transition-colors hover:bg-base-800/35">
                <td className="p-4 text-ink-300">
                  {i === 0 ? (
                    <span className="font-bold text-ink-50">#1</span>
                  ) : i === 1 ? (
                    <span className="font-bold text-ink-100">#2</span>
                  ) : i === 2 ? (
                    <span className="font-bold text-ink-200">#3</span>
                  ) : (
                    `#${i + 1}`
                  )}
                </td>
                <td className="p-4 font-mono text-sm tracking-tight text-ink-100">{row.wallet}</td>
                <td className="p-4 text-right font-mono font-medium text-accent-400">
                  {Number(row.balance).toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} SHRD
                </td>
              </tr>
            ))}
            {data.length === 0 && (
              <tr>
                <td colSpan={3} className="p-12 text-center text-sm italic text-ink-300">
                  Contributor ledger data is unavailable or the network is still syncing.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </main>
  )
}
