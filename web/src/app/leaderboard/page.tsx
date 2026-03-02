"use client"
import { useEffect, useState } from "react"
import { apiUrl } from "@/lib/config"

export default function LeaderboardPage() {
    const [data, setData] = useState<any[]>([])

    useEffect(() => {
        fetch(apiUrl('/v1/leaderboard'))
            .then(r => r.json())
            .then(d => { if (d.ok && d.leaderboard) setData(d.leaderboard) })
            .catch()
    }, [])

    return (
        <div className="container mx-auto p-4 max-w-4xl mt-12 mb-20">
            <h1 className="text-3xl font-bold mb-4 font-mono text-primary">Network Leaderboard</h1>
            <p className="text-muted mb-8 text-sm">Top contributors providing verifiable inference on the Shard network.</p>

            <div className="card overflow-hidden">
                <table className="w-full text-left border-collapse">
                    <thead className="bg-secondary/10 text-secondary text-sm border-b border-secondary/20">
                        <tr>
                            <th className="p-4 font-semibold w-24">Rank</th>
                            <th className="p-4 font-semibold">Wallet</th>
                            <th className="p-4 font-semibold text-right">Credits Earned</th>
                        </tr>
                    </thead>
                    <tbody>
                        {data.map((row, i) => (
                            <tr key={row.wallet} className="border-b border-secondary/10 hover:bg-secondary/5 transition-colors">
                                <td className="p-4 text-muted">
                                    {i === 0 ? <span className="text-amber-400 font-bold">#1 🏆</span> :
                                        i === 1 ? <span className="text-gray-300 font-bold">#2 🥈</span> :
                                            i === 2 ? <span className="text-amber-700 font-bold">#3 🥉</span> :
                                                `#${i + 1}`}
                                </td>
                                <td className="p-4 font-mono text-sm tracking-tight text-emerald-400/90">{row.wallet}</td>
                                <td className="p-4 text-right font-mono text-primary/90 font-medium">
                                    {Number(row.balance).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })} SHRD
                                </td>
                            </tr>
                        ))}
                        {data.length === 0 && (
                            <tr>
                                <td colSpan={3} className="p-12 text-center text-muted text-sm italic">
                                    Leaderboard data unavailable or network is syncing.
                                </td>
                            </tr>
                        )}
                    </tbody>
                </table>
            </div>
        </div>
    )
}
