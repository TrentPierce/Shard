"use client"

import { useEffect, useRef, useState } from "react"
import { useAppContext, type NodeMode } from "@/lib/context"
import { sendMessage, type ChatMessage } from "@/lib/api"
import { useProductSignals } from "@/hooks/useProductSignals"
import { apiUrl } from "@/lib/config"

interface ChatPanelProps {
    mode: NodeMode
}

export default function ChatPanel({ mode }: ChatPanelProps) {
    const { topology } = useAppContext()
    const [messages, setMessages] = useState<ChatMessage[]>([])
    const [input, setInput] = useState("")
    const [streaming, setStreaming] = useState(false)
    const messagesEndRef = useRef<HTMLDivElement>(null)
    const textareaRef = useRef<HTMLTextAreaElement>(null)
    const { health, successRate } = useProductSignals()
    const [inferenceMode, setInferenceMode] = useState<"standard" | "distributed">("distributed")
    const [opsSummary, setOpsSummary] = useState<{ active_nodes?: number }>({})

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
        <main className="chat" aria-label="Shard terminal gateway">
            <div className="chat__header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <div>
                    <h2 className="chat__title" style={{ fontSize: '18px', letterSpacing: '2px' }}>[ NEURAL_GATEWAY_V1.4 ]</h2>
                    <div style={{ display: 'flex', gap: '10px', marginTop: '4px', fontSize: '10px', opacity: 0.6 }}>
                        <span>MODEL: {modelLabel.toUpperCase()}</span>
                        <span>CORE: {versionLabel.toUpperCase()}</span>
                        <select
                            id="inference-mode-select"
                            value={inferenceMode}
                            onChange={(e) => setInferenceMode(e.target.value as "standard" | "distributed")}
                            style={{ background: 'transparent', color: 'inherit', border: 'none', borderBottom: '1px solid var(--border)', fontSize: 'inherit', cursor: 'pointer' }}
                        >
                            <option value="standard">MODE: SINGLE_NODE</option>
                            <option value="distributed">MODE: DIST_MESH</option>
                        </select>
                    </div>
                </div>
                <div style={{ textAlign: 'right' }}>
                    <div className={`stat-value--${ready ? "accent" : "error"}`} style={{ fontSize: '12px', fontWeight: 'bold' }}>
                        {ready ? "STATUS: ONLINE" : "STATUS: SYNCING"}
                    </div>
                    <div style={{ fontSize: '10px', opacity: 0.6 }}>
                        {opsSummary.active_nodes ?? 0} NODES // {successRate}% RELIABILITY
                    </div>
                </div>
            </div>

            <div className="chat__messages" style={{ fontFamily: 'var(--font-mono)' }}>
                {messages.length === 0 ? (
                    <div className="chat__empty" style={{ opacity: 0.8 }}>
                        <pre style={{ color: 'var(--primary)', marginBottom: '20px', fontSize: '12px', lineHeight: '1.2' }}>{`
   _____ _    _          _____  _____  
  / ____| |  | |   /\   |  __ \\|  __ \\ 
 | (___ | |__| |  /  \\  | |__) | |  | |
  \\___ \\|  __  | / /\\ \\ |  _  /| |  | |
  ____) | |  | |/ ____ \\| | \\ \\| |__| |
 |_____/|_|  |_/_/    \\_\\_|  \\_\\_____/ 
                                       
        `}</pre>
                        <h3 className="chat__empty-title" style={{ color: 'var(--primary)', fontSize: '14px' }}>ESTABLISHING_TRUSTLESS_MESH_LINK...</h3>
                        <p className="chat__empty-hint" style={{ fontSize: '12px', color: 'var(--muted)' }}>
                            // ALL COMPUTE IS DECENTRALIZED AND ENCRYPTED
                        </p>
                        <div className="chat__quick-prompts" style={{ marginTop: '20px', borderTop: '1px dashed var(--border)', paddingTop: '20px' }}>
                            {quickPrompts.map((prompt) => (
                                <button
                                    key={prompt}
                                    type="button"
                                    className="chat__quick-btn"
                                    style={{ border: '1px solid var(--border)', background: 'transparent', color: 'var(--muted)', padding: '6px 12px', margin: '4px', cursor: 'pointer', fontSize: '12px' }}
                                    onClick={() => setInput(prompt)}
                                >
                                    {"> "} {prompt}
                                </button>
                            ))}
                        </div>
                    </div>
                ) : (
                    messages.map((msg, i) => (
                        <div
                            key={i}
                            className={`message ${msg.role === "user" ? "message--user" : "message--assistant"}`}
                            style={{ marginBottom: '16px', maxWidth: '100%' }}
                        >
                            <div className="message__avatar" style={{ fontSize: '11px', marginBottom: '4px', display: 'flex', gap: '8px' }}>
                                <span style={{ color: msg.role === 'user' ? 'var(--secondary)' : 'var(--primary)', fontWeight: 'bold' }}>
                                    {msg.role === "user" ? "GUEST@SHARD-NET:~$" : "SYSTEM@SHARD-CORE:~#"}
                                </span>
                                <span style={{ opacity: 0.3 }}>|</span>
                                <span style={{ opacity: 0.5 }}>{new Date(msg.timestamp).toLocaleTimeString()}</span>
                            </div>
                            <div className="message__bubble" style={{
                                background: 'transparent',
                                border: 'none',
                                padding: '0 10px',
                                borderLeft: `2px solid ${msg.role === 'user' ? 'var(--secondary)' : 'var(--primary)'}`,
                                color: msg.role === 'user' ? 'var(--secondary)' : 'var(--primary)',
                                fontSize: '13px'
                            }}>
                                {msg.content || (
                                    <span className="cursor" />
                                )}
                                {streaming && i === messages.length - 1 && msg.role === 'assistant' && (
                                    <span className="cursor" />
                                )}
                            </div>
                        </div>
                    ))
                )}
                <div ref={messagesEndRef} />
            </div>

            <div className="chat__input-area">
                <div className="chat__input-wrapper" style={{ position: 'relative' }}>
                    <span style={{ position: 'absolute', left: '10px', top: '10px', color: 'var(--primary)' }}>&gt;</span>
                    <textarea
                        id="chat-prompt-input"
                        ref={textareaRef}
                        className="chat__input"
                        style={{ paddingLeft: '25px', width: '100%', minHeight: '40px' }}
                        placeholder={mode === "loading" ? "INITIALIZING..." : "COMMAND:"}
                        value={input}
                        onChange={(e) => setInput(e.target.value)}
                        onKeyDown={handleKeyDown}
                        disabled={mode === "loading"}
                        rows={1}
                    />
                    <button
                        className="chat__send-btn"
                        onClick={handleSend}
                        disabled={!input.trim() || streaming || mode === "loading"}
                    >
                        {streaming ? "..." : "[ SEND ]"}
                    </button>
                </div>
                <div style={{ fontSize: '9px', color: 'var(--muted)', marginTop: '8px', textTransform: 'uppercase', letterSpacing: '1px' }}>
                    {streaming ? "WAITING_FOR_SWARM_RESPONSE..." : "READY_FOR_INPUT // SHIFT+ENTER_NEWLINE"}
                </div>
            </div>
        </main>
    )
}
