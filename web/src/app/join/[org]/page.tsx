"use client"

import { useParams } from "next/navigation"
import Header from "@/components/Header"
import Footer from "@/components/Footer"
import { Globe, Shield, Users, ArrowRight } from "lucide-react"
import Link from "next/link"

export default function JoinOrgPage() {
    const params = useParams()
    const org = params.org as string

    return (
        <div className="bg-grid min-h-screen">
            <Header />
            
            <main className="container flex items-center justify-center section">
                <div className="card card-elevated" style={{ maxWidth: '480px', width: '100%', padding: 'var(--space-2xl)' }}>
                    <div className="text-center mb-xl">
                        <div className="avatar-orb avatar-orb--bot mx-auto mb-md" style={{ margin: '0 auto var(--space-md)' }}>
                            <Users size={32} />
                        </div>
                        <h1>Join {org}</h1>
                        <p className="text-secondary mt-sm">
                            Connect your browser to the <strong>{org}</strong> private mesh. 
                            Contribute compute, access shared model pools.
                        </p>
                    </div>

                    <div className="flex flex-col gap-lg">
                        <div className="card card-glass" style={{ padding: 'var(--space-md)' }}>
                            <div className="flex items-center gap-md">
                                <Shield className="text-primary" size={20} />
                                <div>
                                    <h4 style={{ fontSize: '0.9375rem', marginBottom: '2px' }}>Authorized Session</h4>
                                    <p style={{ fontSize: '0.8125rem' }}>Direct P2P encryption enabled</p>
                                </div>
                            </div>
                        </div>

                        <div className="flex items-center justify-between" style={{ fontSize: '0.875rem', color: 'var(--text-muted)' }}>
                            <span>Role: Browser Scout</span>
                            <span>Region: Auto-detect</span>
                        </div>

                        <Link href="/chat" className="btn btn-primary w-full btn-lg">
                            Activate Node
                            <ArrowRight size={18} />
                        </Link>

                        <p style={{ fontSize: '0.75rem', textAlign: 'center', color: 'var(--text-muted)' }}>
                            By joining, your browser will use WebGPU to assist in speculative decoding for this organization.
                        </p>
                    </div>
                </div>
            </main>

            <Footer />

            <style jsx>{`
                .avatar-orb--bot {
                    width: 80px;
                    height: 80px;
                    background: var(--primary-glow);
                    color: var(--primary);
                    border-radius: 50%;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    border: 1px solid rgba(0, 212, 170, 0.2);
                }
            `}</style>
        </div>
    )
}
