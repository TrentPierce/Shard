"use client"

import { useEffect, useState } from "react"
import Link from "next/link"
import { useParams } from "next/navigation"
import ScoutCapabilityBadge from "@/components/ScoutCapabilityBadge"
import { ArrowRight, Cpu, Users } from "lucide-react"

export default function JoinOrgPage() {
    const params = useParams()
    const orgSlug = params?.org as string
    const [scoutCount, setScoutCount] = useState(0)

    // Simulate fetching org details
    const orgName = orgSlug ? orgSlug.charAt(0).toUpperCase() + orgSlug.slice(1).replace("-", " ") : "Your Organization"

    useEffect(() => {
        // Poll for scout count
        const fetchStats = async () => {
            try {
                // In a real app, filter by org tag
                const res = await fetch("/v1/system/topology")
                if (res.ok) {
                    const data = await res.json()
                    setScoutCount(data.scout_count || 0)
                }
            } catch {}
        }
        fetchStats()
    }, [])

    return (
        <div className="min-h-screen flex flex-col items-center justify-center bg-grid p-4">
            <div className="max-w-md w-full bg-card border border-border rounded-xl p-8 shadow-2xl animate-fade-in-up">
                <div className="text-center mb-8">
                    <div className="inline-flex items-center justify-center w-16 h-16 rounded-full bg-primary/10 text-primary mb-4">
                        <Users size={32} />
                    </div>
                    <h1 className="text-2xl font-bold">Join {orgName} AI Network</h1>
                    <p className="text-muted-foreground mt-2">
                        Contribute your browser's idle compute to power internal AI tools.
                    </p>
                </div>

                <div className="space-y-6">
                    <div className="bg-secondary/50 rounded-lg p-4">
                        <h3 className="text-sm font-medium mb-2">System Compatibility</h3>
                        <ScoutCapabilityBadge showDetails={true} size="lg" />
                    </div>

                    <div className="flex items-center justify-between text-sm text-muted-foreground">
                        <span>Current contributors:</span>
                        <span className="font-mono font-bold text-foreground">{scoutCount} active</span>
                    </div>

                    <Link 
                        href="/dashboard?mode=scout" 
                        className="btn btn-primary w-full flex items-center justify-center gap-2 py-3"
                    >
                        <Cpu size={18} />
                        Start Contributing Compute
                        <ArrowRight size={18} />
                    </Link>

                    <p className="text-xs text-center text-muted-foreground">
                        Runs locally in your browser. No data leaves the private network.
                        Safe to run in background.
                    </p>
                </div>
            </div>
        </div>
    )
}
