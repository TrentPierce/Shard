"use client"

import { useEffect, useState } from "react"
import Header from "@/components/Header"
import { apiUrl } from "@/lib/config"
import { Activity, Server, Zap, DollarSign } from "lucide-react"

export default function AdminPage() {
    const [stats, setStats] = useState<any>(null)

    useEffect(() => {
        const fetchStats = async () => {
            try {
                const res = await fetch(apiUrl("/v1/metrics/summary"))
                if (res.ok) {
                    const data = await res.json()
                    setStats(data)
                }
            } catch (e) {
                console.error(e)
            }
        }
        fetchStats()
        const interval = setInterval(fetchStats, 5000)
        return () => clearInterval(interval)
    }, [])

    const savings = stats?.estimated_gpu_savings_usd || 0

    return (
        <div className="min-h-screen bg-background text-foreground">
            <Header />
            <main className="container py-8">
                <h1 className="text-3xl font-bold mb-8">Pilot Administration</h1>

                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
                    <StatCard 
                        title="Active Scouts" 
                        value={stats?.active_nodes || 0} 
                        icon={<Activity />} 
                        desc="Browsers contributing"
                    />
                    <StatCard 
                        title="Requests Served" 
                        value={stats?.tokens_processed_total || 0} 
                        icon={<Server />} 
                        desc="Total tokens generated"
                    />
                    <StatCard 
                        title="Offload Rate" 
                        value={`${Math.round(stats?.offload_percentage_estimate || 0)}%`} 
                        icon={<Zap />} 
                        desc="Work handled by scouts"
                    />
                    <StatCard 
                        title="Est. Savings" 
                        value={`$${savings.toFixed(2)}`} 
                        icon={<DollarSign />} 
                        desc="Vs Cloud GPU cost"
                        highlight
                    />
                </div>

                <div className="bg-card border border-border rounded-xl p-6">
                    <h2 className="text-xl font-semibold mb-4">Live Network Activity</h2>
                    {stats ? (
                        <pre className="bg-secondary/50 p-4 rounded-lg overflow-auto text-xs font-mono">
                            {JSON.stringify(stats, null, 2)}
                        </pre>
                    ) : (
                        <div className="text-muted-foreground">Loading metrics...</div>
                    )}
                </div>
            </main>
        </div>
    )
}

function StatCard({ title, value, icon, desc, highlight }: any) {
    return (
        <div className={`bg-card border ${highlight ? 'border-primary/50 bg-primary/5' : 'border-border'} rounded-xl p-6`}>
            <div className="flex items-center justify-between mb-4">
                <h3 className="text-sm font-medium text-muted-foreground">{title}</h3>
                <div className={`${highlight ? 'text-primary' : 'text-muted-foreground'}`}>
                    {icon}
                </div>
            </div>
            <div className="text-3xl font-bold mb-1">{value}</div>
            <div className="text-xs text-muted-foreground">{desc}</div>
        </div>
    )
}
