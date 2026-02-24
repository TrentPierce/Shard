"use client"

import { useEffect, useState } from "react"
import Header from "@/components/Header"
import Footer from "@/components/Footer"
import { apiUrl } from "@/lib/config"
import { Activity, Server, Zap, DollarSign, Shield, LineChart } from "lucide-react"

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
        <div className="bg-grid min-h-screen">
            <Header />
            
            <main className="container section">
                <section className="text-center mb-4xl">
                    <span className="hero-eyebrow">Enterprise Control Plane</span>
                    <h1>Pilot Administration</h1>
                    <p className="max-w-2xl mx-auto mt-md text-secondary">
                        Manage your private mesh and monitor cost-savings in real-time. 
                        Verified compute metrics for stakeholders and infrastructure leads.
                    </p>
                </section>

                <div className="grid grid-4 mb-2xl">
                    <StatCard 
                        title="Active Scouts" 
                        value={stats?.active_nodes || 0} 
                        icon={<Activity size={20} />} 
                        desc="Browsers contributing"
                    />
                    <StatCard 
                        title="Tokens Served" 
                        value={stats?.tokens_processed_total?.toLocaleString() || 0} 
                        icon={<Server size={20} />} 
                        desc="Total network output"
                    />
                    <StatCard 
                        title="Efficiency" 
                        value={`${Math.round(stats?.offload_percentage_estimate || 0)}%`} 
                        icon={<Zap size={20} />} 
                        desc="Offload to browser tier"
                    />
                    <StatCard 
                        title="Gains (USD)" 
                        value={`$${savings.toFixed(2)}`} 
                        icon={<DollarSign size={20} />} 
                        desc="Est. Cloud GPU savings"
                        highlight
                    />
                </div>

                <div className="grid grid-2">
                    <div className="card bg-elevated">
                        <h2 className="flex items-center gap-md mb-xl" style={{ fontSize: '1.25rem' }}>
                            <Shield size={20} className="text-primary" />
                            Governance Policies
                        </h2>
                        <div className="flex flex-col gap-md">
                            <div className="card card-glass" style={{ padding: 'var(--space-md)' }}>
                                <div className="flex justify-between items-center">
                                    <span className="font-bold">Public Mesh Peering</span>
                                    <span className="badge badge-primary">ENABLED</span>
                                </div>
                            </div>
                            <div className="card card-glass" style={{ padding: 'var(--space-md)' }}>
                                <div className="flex justify-between items-center">
                                    <span className="font-bold">Authorized Domains Only</span>
                                    <span className="badge badge-secondary">DISABLED</span>
                                </div>
                            </div>
                            <div className="card card-glass" style={{ padding: 'var(--space-md)' }}>
                                <div className="flex justify-between items-center">
                                    <span className="font-bold">Proof-of-Compute Staking</span>
                                    <span className="badge" style={{ background: 'rgba(255,255,255,0.05)', color: 'var(--text-muted)' }}>PHASE 5</span>
                                </div>
                            </div>
                        </div>
                    </div>

                    <div className="card h-full">
                        <h2 className="flex items-center gap-md mb-xl" style={{ fontSize: '1.25rem' }}>
                            <LineChart size={20} className="text-primary" />
                            Live Network Activity
                        </h2>
                        {stats ? (
                            <div className="code-block" style={{ height: '300px', overflowY: 'auto' }}>
                                <code>
                                    {JSON.stringify(stats, null, 2)}
                                </code>
                            </div>
                        ) : (
                            <div className="text-center py-4xl text-muted animate-pulse">
                                Awaiting telemetry heartbeat...
                            </div>
                        )}
                    </div>
                </div>
            </main>

            <Footer />
        </div>
    )
}

function StatCard({ title, value, icon, desc, highlight }: any) {
    return (
        <div className={`card ${highlight ? 'animate-glow' : ''}`} style={{ borderColor: highlight ? 'var(--primary)' : 'var(--border)' }}>
            <div className="flex items-center justify-between mb-md">
                <h3 style={{ fontSize: '0.875rem', fontWeight: 600, color: 'var(--text-secondary)', marginBottom: 0 }}>{title}</h3>
                <div style={{ color: highlight ? 'var(--primary)' : 'var(--text-muted)' }}>
                    {icon}
                </div>
            </div>
            <div style={{ fontSize: '2rem', fontWeight: 800, marginBottom: 'var(--space-xs)' }}>{value}</div>
            <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>{desc}</div>
        </div>
    )
}
