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
    <main id="main-content" className="mx-auto mb-20 mt-12 w-full max-w-4xl px-4">
      <h1 className="mb-4 text-3xl font-bold text-ink-50">Network Leaderboard</h1>
      <p className="mb-8 text-sm text-ink-300">Top contributors providing verifiable inference on the Shard network.</p>

      <div className="overflow-hidden rounded-2xl border border-ring bg-base-900">
        <table className="w-full border-collapse text-left">
          <thead className="border-b border-ring bg-base-800/40 text-sm text-ink-200">
            <tr>
              <th className="w-24 p-4 font-semibold">Rank</th>
              <th className="p-4 font-semibold">Wallet</th>
              <th className="p-4 text-right font-semibold">Credits Earned</th>
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
                  Leaderboard data unavailable or network is syncing.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </main>
  )
}
