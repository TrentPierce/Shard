"use client"

import { useEffect, useRef, useState } from "react"
import { useAppContext, type NodeMode } from "@/lib/context"
import { sendMessage, type ChatMessage } from "@/lib/api"
import { useProductSignals } from "@/hooks/useProductSignals"
import { apiUrl } from "@/lib/config"

interface ChatPanelProps {
    mode: NodeMode
}

import { Terminal, Activity, Cpu, Send, MessageSquare, Sparkles, AlertCircle } from "lucide-react"

export default function ChatPanel({ mode }: ChatPanelProps) {
    const { topology, theme } = useAppContext()
    const [messages, setMessages] = useState<ChatMessage[]>([])
    const [input, setInput] = useState("")
    const [streaming, setStreaming] = useState(false)
    const messagesEndRef = useRef<HTMLDivElement>(null)
    const textareaRef = useRef<HTMLTextAreaElement>(null)
    const { health, successRate } = useProductSignals()
    const [inferenceMode, setInferenceMode] = useState<"standard" | "distributed">("distributed")
    const [opsSummary, setOpsSummary] = useState<{ active_nodes?: number }>({})

    const isEnterprise = theme === "enterprise"
    const modelLabel = topology?.model_id ?? "default-model"

    const scrollToBottom = () => {
        messagesEndRef.current?.scrollIntoView({ behavior: "smooth" })
    }

    useEffect(() => {
        scrollToBottom()
    }, [messages])

    useEffect(() => {
        const el = textareaRef.current
        if (el) {
            el.style.height = "24px"
            el.style.height = `${Math.min(el.scrollHeight, 120)}px`
        }
    }, [input])

    useEffect(() => {
        let cancelled = false
        const poll = async () => {
            try {
                const res = await fetch(apiUrl("/v1/metrics/summary"), { cache: "no-store" })
                if (!res.ok) return
                const data = await res.json()
                if (!cancelled) setOpsSummary(data ?? {})
            } catch {
                // ignore telemetry polling failures
            }
        }

        poll()
        const timer = setInterval(poll, 5000)
        return () => {
            cancelled = true
            clearInterval(timer)
        }
    }, [])

    const handleSend = async () => {
        const text = input.trim()
        if (!text || streaming) return

        const userMsg: ChatMessage = {
            role: "user",
            content: text,
            timestamp: Date.now(),
        }

        setMessages((prev) => [...prev, userMsg])
        setInput("")
        setStreaming(true)

        const assistantMsg: ChatMessage = {
            role: "assistant",
            content: "",
            timestamp: Date.now(),
        }
        setMessages((prev) => [...prev, assistantMsg])

        try {
            await sendMessage(
                [...messages, userMsg],
                (token) => {
                    setMessages((prev) => {
                        const updated = [...prev]
                        const last = updated[updated.length - 1]
                        if (last.role === "assistant") {
                            updated[updated.length - 1] = {
                                ...last,
                                content: last.content + token,
                            }
                        }
                        return updated
                    })
                },
                () => {
                    setStreaming(false)
                },
                inferenceMode,
            )
        } catch (err: any) {
            if (typeof window !== "undefined") {
                window.dispatchEvent(new Event("shard:chat-failure"))
            }
            setMessages((prev) => {
                const updated = [...prev]
                const last = updated[updated.length - 1]
                if (last.role === "assistant") {
                    updated[updated.length - 1] = {
                        ...last,
                        content:
                            "Network unavailable: " +
                            (err?.message ?? "Could not connect to the local Shard daemon."),
                    }
                }
                return updated
            })
            setStreaming(false)
        }
    }

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault()
            handleSend()
        }
    }

    const ready = mode !== "loading"
    const versionLabel = health.rust_version ? `v${health.rust_version}` : "v0.0.0"

    const quickPrompts = [
        "How does distributed inference work?",
        "What is the current network health?",
        "Explain Shard architecture.",
    ]

    return (
        <main className={`chat ${isEnterprise ? 'theme-enterprise' : ''}`} aria-label="Shard gateway">
            <div className={`chat__header ${isEnterprise ? 'glass-card border-none' : ''}`} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '16px 24px' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
                    {isEnterprise ? (
                        <div className="w-10 h-10 rounded-full bg-secondary/10 flex items-center justify-center text-secondary">
                            <Sparkles size={20} />
                        </div>
                    ) : null}
                    <div>
                        <h2 className="chat__title" style={{ fontSize: isEnterprise ? '16px' : '18px', letterSpacing: isEnterprise ? 'noral' : '2px', fontWeight: isEnterprise ? '600' : 'bold' }}>
                            {isEnterprise ? "Neural Assistant" : "[ NEURAL_GATEWAY_V1.4 ]"}
                        </h2>
                        <div style={{ display: 'flex', gap: '12px', marginTop: '4px', fontSize: '11px', opacity: 0.7 }}>
                            <span className="flex items-center gap-1"><Cpu size={12} /> {modelLabel.toUpperCase()}</span>
                            <span className="flex items-center gap-1"><Activity size={12} /> {versionLabel.toUpperCase()}</span>
                            <select
                                id="inference-mode-select"
                                value={inferenceMode}
                                onChange={(e) => setInferenceMode(e.target.value as "standard" | "distributed")}
                                style={{ background: 'transparent', color: 'inherit', border: 'none', borderBottom: isEnterprise ? 'none' : '1px solid var(--border)', fontSize: 'inherit', cursor: 'pointer', fontWeight: '500' }}
                            >
                                <option value="standard">{isEnterprise ? "Standard Mode" : "MODE: SINGLE_NODE"}</option>
                                <option value="distributed">{isEnterprise ? "Distributed Mesh" : "MODE: DIST_MESH"}</option>
                            </select>
                        </div>
                    </div>
                </div>
                <div style={{ textAlign: 'right' }}>
                    <div className={ready ? "text-accent-emerald" : "text-error"} style={{ fontSize: '12px', fontWeight: '700' }}>
                        {ready ? (isEnterprise ? "CONNECTED" : "STATUS: ONLINE") : (isEnterprise ? "SYNCING..." : "STATUS: SYNCING")}
                    </div>
                    <div style={{ fontSize: '10px', opacity: 0.6, marginTop: '2px' }}>
                        {opsSummary.active_nodes ?? 0} NODES • {successRate}% RELIABILITY
                    </div>
                </div>
            </div>

            <div className="chat__messages" style={{ fontFamily: isEnterprise ? 'var(--font-sans)' : 'var(--font-mono)', padding: '24px' }}>
                {messages.length === 0 ? (
                    <div className="chat__empty" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', textAlign: 'center' }}>
                        {!isEnterprise ? (
                            <pre style={{ color: 'var(--primary)', marginBottom: '20px', fontSize: '12px', lineHeight: '1.2' }}>{`
   _____ _    _          _____  _____  
  / ____| |  | |   /\   |  __ \\|  __ \\ 
 | (___ | |__| |  /  \\  | |__) | |  | |
  \\___ \\|  __  | / /\\ \\ |  _  /| |  | |
  ____) | |  | |/ ____ \\| | \\ \\| |__| |
 |_____/|_|  |_/_/    \\_\\_|  \\_\\_____/ 
                                       
        `}</pre>
                        ) : (
                            <div className="w-16 h-16 rounded-2xl bg-secondary/10 flex items-center justify-center text-secondary mb-6">
                                <MessageSquare size={32} />
                            </div>
                        )}
                        <h3 className="chat__empty-title" style={{ color: 'var(--text-primary)', fontSize: isEnterprise ? '24px' : '14px', fontWeight: '700' }}>
                            {isEnterprise ? "What can I help you build?" : "ESTABLISHING_TRUSTLESS_MESH_LINK..."}
                        </h3>
                        <p className="chat__empty-hint" style={{ fontSize: '14px', color: 'var(--text-muted)', marginTop: '8px', maxWidth: '400px' }}>
                            {isEnterprise
                                ? "Ask anything. All compute is distributed across the Shard decentralized network for private, local-first inference."
                                : "// ALL COMPUTE IS DECENTRALIZED AND ENCRYPTED"}
                        </p>
                        <div className="chat__quick-prompts" style={{ marginTop: '32px', display: 'flex', flexWrap: 'wrap', justifyContent: 'center', gap: '8px' }}>
                            {quickPrompts.map((prompt) => (
                                <button
                                    key={prompt}
                                    type="button"
                                    className={isEnterprise ? "glass-card hover:bg-white/5 transition-all px-4 py-2 border-none rounded-full normal-case text-sm" : ""}
                                    style={!isEnterprise ? { border: '1px solid var(--border)', background: 'transparent', color: 'var(--muted)', padding: '6px 12px', margin: '4px', cursor: 'pointer', fontSize: '12px' } : {}}
                                    onClick={() => setInput(prompt)}
                                >
                                    {!isEnterprise ? "> " : ""}{prompt}
                                </button>
                            ))}
                        </div>
                    </div>
                ) : (
                    messages.map((msg, i) => (
                        <div
                            key={i}
                            className={`message ${msg.role === "user" ? "message--user" : "message--assistant"}`}
                            style={{ marginBottom: '24px', maxWidth: isEnterprise ? '85%' : '100%' }}
                        >
                            <div className="message__avatar" style={{ fontSize: '11px', marginBottom: '6px', display: 'flex', gap: '8px', alignItems: 'center' }}>
                                <span style={{ color: msg.role === 'user' ? 'var(--secondary)' : 'var(--primary)', fontWeight: '700' }}>
                                    {isEnterprise
                                        ? (msg.role === "user" ? "YOU" : "SHARD AI")
                                        : (msg.role === "user" ? "GUEST@SHARD-NET:~$" : "SYSTEM@SHARD-CORE:~#")}
                                </span>
                                <span style={{ opacity: 0.2 }}>•</span>
                                <span style={{ opacity: 0.5 }}>{new Date(msg.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                            </div>
                            <div className={`message__bubble ${isEnterprise ? 'glass-card' : ''}`} style={{
                                background: isEnterprise ? (msg.role === 'user' ? 'rgba(0,112,243,0.1)' : 'var(--glass-bg)') : 'transparent',
                                border: isEnterprise ? 'var(--border-width) solid var(--glass-border)' : 'none',
                                padding: isEnterprise ? '12px 16px' : '0 10px',
                                borderLeft: !isEnterprise ? `2px solid ${msg.role === 'user' ? 'var(--secondary)' : 'var(--primary)'}` : 'var(--border-width) solid var(--glass-border)',
                                color: isEnterprise ? 'var(--text-primary)' : (msg.role === 'user' ? 'var(--secondary)' : 'var(--primary)'),
                                fontSize: '15px',
                                lineHeight: '1.6',
                                borderRadius: isEnterprise ? '12px' : '0'
                            }}>
                                {msg.content || (
                                    <span className="animate-pulse">●</span>
                                )}
                                {streaming && i === messages.length - 1 && msg.role === 'assistant' && (
                                    <span className={isEnterprise ? "animate-pulse ml-1" : "cursor"}>
                                        {isEnterprise ? '●' : ''}
                                    </span>
                                )}
                            </div>
                        </div>
                    ))
                )}
                <div ref={messagesEndRef} />
            </div>

            <div className={`chat__input-area ${isEnterprise ? 'bg-transparent' : ''}`}>
                <div className={`chat__input-wrapper ${isEnterprise ? 'glass-card border-none rounded-2xl p-2' : ''}`} style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
                    {!isEnterprise && <span style={{ position: 'absolute', left: '10px', color: 'var(--primary)' }}>&gt;</span>}
                    <textarea
                        id="chat-prompt-input"
                        ref={textareaRef}
                        className="chat__input"
                        style={{ paddingLeft: isEnterprise ? '12px' : '25px', width: '100%', minHeight: '44px', maxHeight: '200px', flex: 1, resize: 'none', alignSelf: 'center' }}
                        placeholder={mode === "loading" ? "Initializing..." : (isEnterprise ? "Ask anything..." : "COMMAND:")}
                        value={input}
                        onChange={(e) => setInput(e.target.value)}
                        onKeyDown={handleKeyDown}
                        disabled={mode === "loading"}
                        rows={1}
                    />
                    <button
                        className={`flex items-center justify-center transition-all ${isEnterprise ? 'bg-secondary text-white w-10 h-10 rounded-xl border-none' : 'chat__send-btn'}`}
                        onClick={handleSend}
                        disabled={!input.trim() || streaming || mode === "loading"}
                        style={isEnterprise ? { padding: 0 } : {}}
                    >
                        {streaming ? <div className="animate-spin h-4 w-4 border-2 border-white/20 border-t-white rounded-full" /> : (isEnterprise ? <Send size={18} /> : "[ SEND ]")}
                    </button>
                </div>
                <div style={{ fontSize: '10px', color: 'var(--text-muted)', marginTop: '12px', textTransform: 'uppercase', letterSpacing: '1px', textAlign: 'center' }}>
                    {streaming ? (isEnterprise ? "Assistant is thinking..." : "WAITING_FOR_SWARM_RESPONSE...") : (isEnterprise ? "Private & Decentralized" : "READY_FOR_INPUT // SHIFT+ENTER_NEWLINE")}
                </div>
            </div>
        </main>
    )
}
