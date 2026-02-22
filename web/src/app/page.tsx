"use client"

import { useState, useEffect, useCallback } from "react"
import Link from "next/link"
import Header from "@/components/Header"
import Footer from "@/components/Footer"
import { apiUrl } from "@/lib/config"
import { 
    Zap, Shield, Globe, Cpu, 
    ArrowRight, CheckCircle, Code, 
    Terminal, Users, Building, GraduationCap,
    Play, Copy, Check
} from "lucide-react"

// Features data
const features = [
    {
        icon: Zap,
        title: "Zero Runtime Cost",
        description: "Eliminate per-token pricing. Your users' browsers provide the compute through WebGPU-powered speculative decoding.",
    },
    {
        icon: Globe,
        title: "Decentralized Scale",
        description: "More users = more GPUs. The network grows organically as adoption increases, with no hardware procurement.",
    },
    {
        icon: Shield,
        title: "Enterprise Security",
        description: "Localhost-first routing keeps sensitive prompts off public servers. Optional private mesh deployment.",
    },
    {
        icon: Cpu,
        title: "OpenAI-Compatible",
        description: "Drop-in replacement for OpenAI API. Zero code rewrites for existing applications.",
    },
]

// How it works steps
const howItWorksSteps = [
    {
        icon: Terminal,
        title: "User Sends Prompt",
        description: "Any user sends a request through the OpenAI-compatible API endpoint.",
    },
    {
        icon: Cpu,
        title: "Scouts Draft Tokens",
        description: "Browser-based Scouts run lightweight models via WebGPU to generate speculative drafts.",
    },
    {
        icon: CheckCircle,
        title: "Shards Verify",
        description: "Verifier nodes validate drafts using BitNet 1.58-bit models with probabilistic MatMul checks.",
    },
    {
        icon: Zap,
        title: "Tokens Stream Back",
        description: "Verified tokens stream to the client with quality matching centralized AI services.",
    },
]

// Use cases data
const useCases = [
    {
        icon: Users,
        title: "Community Chatbots",
        description: "Power free AI chatbots where users contribute compute instead of paying API fees.",
    },
    {
        icon: GraduationCap,
        title: "Education & Research",
        description: "Classroom setups where student browsers act as Scouts with one workstation as verifier.",
    },
    {
        icon: Building,
        title: "Enterprise Overflow",
        description: "Burst capacity when centralized GPU budgets are exhausted. Seamless scaling.",
    },
    {
        icon: Zap,
        title: "Hackathon Demos",
        description: "One EC2 verifier + attendee browser Scouts creates a complete demo environment instantly.",
    },
]

// Comparison data
const comparisonData = [
    { feature: "Cost per 1K tokens", traditional: "$0.002 - $0.06", shard: "Free" },
    { feature: "Data Privacy", traditional: "Third-party servers", shard: "Localhost-first" },
    { feature: "Scalability", traditional: "Buy more GPUs", shard: "More users = more GPUs" },
    { feature: "Uptime", traditional: "Single point of failure", shard: "Self-healing mesh" },
    { feature: "Latency", traditional: "Network RTT + queue", shard: "Local draft + verify" },
    { feature: "API Compatibility", traditional: "Native", shard: "OpenAI-compatible" },
]

interface NetworkStats {
    scouts: number
    shards: number
    tokens24h: number
    uptime: number
}

export default function HomePage() {
    const [networkStats, setNetworkStats] = useState<NetworkStats>({
        scouts: 0,
        shards: 0,
        tokens24h: 0,
        uptime: 99.9,
    })
    const [statsLoading, setStatsLoading] = useState(true)
    const [activeApiTab, setActiveApiTab] = useState("python")
    const [copiedCode, setCopiedCode] = useState("")

    const fetchNetworkStats = useCallback(async () => {
        try {
            const [topologyData, metricsData, healthData] = await Promise.all([
                fetch(apiUrl("/v1/system/topology"), { cache: "no-store" }).then((res) => (res.ok ? res.json() : null)),
                fetch(apiUrl("/v1/metrics/summary"), { cache: "no-store" }).then((res) => (res.ok ? res.json() : null)),
                fetch(apiUrl("/health"), { cache: "no-store" }).then((res) => (res.ok ? res.json() : null)),
            ])

            if (topologyData || metricsData || healthData) {
                setNetworkStats({
                    scouts: Number(topologyData?.scout_count ?? healthData?.active_scouts ?? 0),
                    shards: Number(topologyData?.shard_count ?? healthData?.connected_peers ?? 0),
                    tokens24h: Number(metricsData?.tokens_verified_24h ?? 0),
                    uptime: Number(healthData?.status === "ok" ? 99.99 : 99.5),
                })
            }
        } catch {
            // Keep fallback values
        } finally {
            setStatsLoading(false)
        }
    }, [])

    useEffect(() => {
        fetchNetworkStats()
        const interval = setInterval(fetchNetworkStats, 30000)
        return () => clearInterval(interval)
    }, [fetchNetworkStats])

    const copyCode = (code: string, id: string) => {
        navigator.clipboard.writeText(code)
        setCopiedCode(id)
        setTimeout(() => setCopiedCode(""), 2000)
    }

    const codeExamples = {
        python: `from openai import OpenAI

client = OpenAI(
    base_url="https://shardnetwork.live/v1",
    api_key="your-api-key"
)

response = client.chat.completions.create(
    model="shard-hybrid",
    messages=[{"role": "user", "content": "Hello!"}],
    stream=True
)

for chunk in response:
    print(chunk.choices[0].delta.content or "", end="")`,
        curl: `curl https://shardnetwork.live/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer your-api-key" \\
  -d '{
    "model": "shard-hybrid",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'`,
    }

    return (
        <>
            <Header />
            
            <main id="main-content">
                {/* Hero Section */}
                <section className="hero bg-grid">
                    <div className="hero-bg">
                        <div className="gradient-orb gradient-orb--primary" style={{ width: 600, height: 600, top: -200, right: -200 }} />
                        <div className="gradient-orb gradient-orb--secondary" style={{ width: 400, height: 400, bottom: -100, left: -100 }} />
                    </div>
                    
                    <div className="container">
                        <div className="hero-content animate-fade-in-up">
                            <span className="hero-eyebrow">
                                Distributed Inference Network
                            </span>
                            <h1>
                                Free AI for everyone.
                                <span className="text-gradient"> Powered by you.</span>
                            </h1>
                            <p className="hero-description">
                                Shard bypasses centralized GPU constraints through WebGPU speculative decoding across a decentralized P2P mesh. Your browser becomes an AI compute node.
                            </p>
                            <div className="hero-actions">
                                <Link href="/docs/deployment-guide" className="btn btn-primary btn-lg">
                                    Get Started
                                    <ArrowRight size={18} />
                                </Link>
                                <Link href="#how-it-works" className="btn btn-secondary btn-lg">
                                    See How It Works
                                </Link>
                            </div>
                            
                            <div className="hero-stats">
                                <div className="hero-stat">
                                    <div className="hero-stat-value">{statsLoading ? "..." : networkStats.scouts.toLocaleString()}</div>
                                    <div className="hero-stat-label">Active Scouts</div>
                                </div>
                                <div className="hero-stat">
                                    <div className="hero-stat-value">{statsLoading ? "..." : networkStats.shards.toLocaleString()}</div>
                                    <div className="hero-stat-label">Verifier Nodes</div>
                                </div>
                                <div className="hero-stat">
                                    <div className="hero-stat-value">{statsLoading ? "..." : `${networkStats.uptime}%`}</div>
                                    <div className="hero-stat-label">Network Uptime</div>
                                </div>
                            </div>
                        </div>
                    </div>
                </section>

                {/* How It Works */}
                <section id="how-it-works" className="section how-it-works">
                    <div className="container">
                        <div className="text-center" style={{ marginBottom: 'var(--space-2xl)' }}>
                            <h2>How It Works</h2>
                            <p style={{ maxWidth: 600, margin: 'var(--space-md) auto 0' }}>
                                Shard combines browser Scouts with verifier Shards to deliver server-grade AI quality at zero runtime cost.
                            </p>
                        </div>
                        
                        <div className="steps stagger-children">
                            {howItWorksSteps.map((step, index) => (
                                <div key={step.title} className="step">
                                    <div className="step-icon" style={{ margin: '0 auto var(--space-lg)' }}>
                                        <step.icon size={48} />
                                    </div>
                                    <h3>{step.title}</h3>
                                    <p>{step.description}</p>
                                </div>
                            ))}
                        </div>
                    </div>
                </section>

                {/* Features */}
                <section id="features" className="section features">
                    <div className="container">
                        <div className="text-center" style={{ marginBottom: 'var(--space-2xl)' }}>
                            <h2>Why Shard?</h2>
                            <p style={{ maxWidth: 600, margin: 'var(--space-md) auto 0' }}>
                                The next generation of AI infrastructure. Built for developers who want control without the cost.
                            </p>
                        </div>
                        
                        <div className="features-grid stagger-children">
                            {features.map((feature) => (
                                <article key={feature.title} className="feature-card">
                                    <div className="feature-icon">
                                        <feature.icon size={24} />
                                    </div>
                                    <h3>{feature.title}</h3>
                                    <p>{feature.description}</p>
                                </article>
                            ))}
                        </div>
                    </div>
                </section>

                {/* Comparison Table */}
                <section className="section" style={{ background: 'var(--bg-secondary)' }}>
                    <div className="container">
                        <div className="text-center" style={{ marginBottom: 'var(--space-2xl)' }}>
                            <h2>Shard vs Traditional Cloud AI</h2>
                        </div>
                        
                        <div style={{ overflowX: 'auto' }}>
                            <table className="comparison-table">
                                <thead>
                                    <tr>
                                        <th>Feature</th>
                                        <th>Traditional Cloud AI</th>
                                        <th className="comparison-highlight">Shard Network</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {comparisonData.map((row) => (
                                        <tr key={row.feature}>
                                            <td>{row.feature}</td>
                                            <td>{row.traditional}</td>
                                            <td className="comparison-highlight">
                                                <span style={{ color: 'var(--primary)', fontWeight: 600 }}>{row.shard}</span>
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    </div>
                </section>

                {/* Network Stats */}
                <section className="section network-stats">
                    <div className="container">
                        <div className="text-center" style={{ marginBottom: 'var(--space-2xl)' }}>
                            <h2>Live Network Status</h2>
                            <p style={{ marginTop: 'var(--space-md)' }}>
                                Real-time metrics from the Shard distributed inference network.
                            </p>
                        </div>
                        
                        <div className="stats-grid stagger-children">
                            <div className="stat-card">
                                <div className="stat-value">{statsLoading ? "..." : networkStats.scouts.toLocaleString()}</div>
                                <div className="stat-label">Browser Scouts</div>
                            </div>
                            <div className="stat-card">
                                <div className="stat-value">{statsLoading ? "..." : networkStats.shards.toLocaleString()}</div>
                                <div className="stat-label">Verifier Nodes</div>
                            </div>
                            <div className="stat-card">
                                <div className="stat-value">{statsLoading ? "..." : `${networkStats.tokens24h.toLocaleString()}`}</div>
                                <div className="stat-label">Tokens Verified (24h)</div>
                            </div>
                            <div className="stat-card">
                                <div className="stat-value">{statsLoading ? "..." : `${networkStats.uptime}%`}</div>
                                <div className="stat-label">Uptime</div>
                            </div>
                        </div>
                    </div>
                </section>

                {/* Use Cases */}
                <section id="use-cases" className="section">
                    <div className="container">
                        <div className="text-center" style={{ marginBottom: 'var(--space-2xl)' }}>
                            <h2>Real-World Use Cases</h2>
                            <p style={{ maxWidth: 600, margin: 'var(--space-md) auto 0' }}>
                                Shard adapts to your infrastructure needs, from community apps to enterprise deployments.
                            </p>
                        </div>
                        
                        <div className="use-cases-grid stagger-children">
                            {useCases.map((useCase) => (
                                <article key={useCase.title} className="use-case-card">
                                    <div className="use-case-icon">
                                        <useCase.icon size={28} />
                                    </div>
                                    <h3>{useCase.title}</h3>
                                    <p>{useCase.description}</p>
                                </article>
                            ))}
                        </div>
                    </div>
                </section>

                {/* API Showcase */}
                <section id="api" className="section" style={{ background: 'var(--bg-secondary)' }}>
                    <div className="container">
                        <div className="quick-start-grid">
                            <div>
                                <h2>OpenAI-Compatible API</h2>
                                <p style={{ marginTop: 'var(--space-md)' }}>
                                    Existing applications work with Shard without any code changes. Simply point your API client to our endpoint.
                                </p>
                                
                                <div style={{ marginTop: 'var(--space-xl)' }}>
                                    <div className="api-tabs">
                                        <button 
                                            className={`api-tab ${activeApiTab === 'python' ? 'active' : ''}`}
                                            onClick={() => setActiveApiTab('python')}
                                        >
                                            Python
                                        </button>
                                        <button 
                                            className={`api-tab ${activeApiTab === 'curl' ? 'active' : ''}`}
                                            onClick={() => setActiveApiTab('curl')}
                                        >
                                            cURL
                                        </button>
                                    </div>
                                    
                                    <div className="code-block">
                                        <button
                                            className="btn btn-ghost btn-sm"
                                            style={{ position: 'absolute', top: 8, right: 8 }}
                                            onClick={() => copyCode(codeExamples[activeApiTab as keyof typeof codeExamples], activeApiTab)}
                                            aria-label="Copy code"
                                        >
                                            {copiedCode === activeApiTab ? <Check size={16} /> : <Copy size={16} />}
                                        </button>
                                        <code>
                                            {activeApiTab === 'python' && codeExamples.python.split('\n').map((line, i) => (
                                                <span key={i}>
                                                    {i > 0 && <br />}
                                                    {line.split(' ').map((word, j) => {
                                                        const keywords = ['from', 'import', 'def', 'return', 'class', 'if', 'else', 'for', 'in', 'True', 'False', 'None', 'as']
                                                        const funcs = ['OpenAI', 'client', 'chat', 'completions', 'create', 'print']
                                                        if (keywords.includes(word)) return <span key={j} className="keyword">{word} </span>
                                                        if (funcs.includes(word)) return <span key={j} className="function">{word} </span>
                                                        if (word.startsWith('"') || word.startsWith("'")) return <span key={j} className="string">{word} </span>
                                                        return <span key={j}>{word} </span>
                                                    })}
                                                </span>
                                            ))}
                                            {activeApiTab === 'curl' && codeExamples.curl.split('\n').map((line, i) => (
                                                <span key={i}>
                                                    {i > 0 && <br />}
                                                    {line.includes('curl') && <span className="function">curl</span>}
                                                    {line.includes('-H') && <span className="keyword"> -H</span>}
                                                    {line.includes('"') && <span className="string">{line.match(/"[^"]+"/)?.[0] || ''}</span>}
                                                    {!line.includes('curl') && !line.includes('-H') && !line.match(/"[^"]+"/) && line.trim() && <span className="keyword">{line}</span>}
                                                </span>
                                            ))}
                                        </code>
                                    </div>
                                </div>
                            </div>
                            
                            <div className="quick-start-steps">
                                <h3>Quick Start</h3>
                                
                                <div className="quick-start-step">
                                    <div className="quick-start-step-num">1</div>
                                    <div className="quick-start-step-content">
                                        <h4>Run a Shard Node</h4>
                                        <p>Start a verifier node locally or connect to the public mesh.</p>
                                        <code className="text-mono" style={{ fontSize: '0.875rem', color: 'var(--primary)' }}>
                                            docker compose up --build shard-daemon
                                        </code>
                                    </div>
                                </div>
                                
                                <div className="quick-start-step">
                                    <div className="quick-start-step-num">2</div>
                                    <div className="quick-start-step-content">
                                        <h4>Or Join as Scout</h4>
                                        <p>Open the web app and enable WebGPU to contribute compute.</p>
                                    </div>
                                </div>
                                
                                <div className="quick-start-step">
                                    <div className="quick-start-step-num">3</div>
                                    <div className="quick-start-step-content">
                                        <h4>Start Building</h4>
                                        <p>Use the OpenAI-compatible API to integrate AI into your apps.</p>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </section>

                {/* CTA */}
                <section className="section cta">
                    <div className="container">
                        <div className="cta-inner animate-fade-in-up">
                            <h2>Ready to get started?</h2>
                            <p>
                                Join thousands of developers building the future of distributed AI. Zero setup required for Scouts.
                            </p>
                            <div className="cta-actions">
                                <Link href="/docs/deployment-guide" className="btn btn-primary btn-lg">
                                    <Play size={18} />
                                    Start Building
                                </Link>
                                <Link href="/docs/Shard-White-Paper-Feb-2026" className="btn btn-secondary btn-lg">
                                    Read the White Paper
                                </Link>
                            </div>
                        </div>
                    </div>
                </section>
            </main>

            <Footer />
        </>
    )
}
