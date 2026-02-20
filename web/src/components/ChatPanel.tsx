"use client"

import { useEffect, useRef, useState } from "react"
import type { NodeMode } from "@/lib/context"
import { sendMessage, type ChatMessage } from "@/lib/api"
import { useProductSignals } from "@/hooks/useProductSignals"
import { apiUrl } from "@/lib/config"

interface ChatPanelProps {
    mode: NodeMode
}

export default function ChatPanel({ mode }: ChatPanelProps) {
    const [messages, setMessages] = useState<ChatMessage[]>([])
    const [input, setInput] = useState("")
    const [streaming, setStreaming] = useState(false)
    const messagesEndRef = useRef<HTMLDivElement>(null)
    const textareaRef = useRef<HTMLTextAreaElement>(null)
    const { health, analytics, successRate } = useProductSignals()
    const [inferenceMode, setInferenceMode] = useState<"standard" | "distributed">("distributed")
    const [opsSummary, setOpsSummary] = useState<{ active_nodes?: number; offload_percentage_estimate?: number; estimated_gpu_savings_percent?: number; p95_latency_ms?: number }>({})
    const [modelLabel, setModelLabel] = useState("default-model")

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
            el.style.height = Math.min(el.scrollHeight, 120) + "px"
        }
    }, [input])

    useEffect(() => {
        let cancelled = false
        const poll = async () => {
            try {
                const res = await fetch(apiUrl("/metrics/summary"), { cache: "no-store" })
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

    useEffect(() => {
        let cancelled = false
        const pollModel = async () => {
            try {
                const res = await fetch(apiUrl("/v1/system/topology"), { cache: "no-store" })
                if (!res.ok) return
                const data = await res.json()
                const modelId = typeof data?.model_id === "string" ? data.model_id.trim() : ""
                if (!cancelled && modelId) {
                    setModelLabel(modelId)
                }
            } catch {
                // ignore topology polling failures
            }
        }
        pollModel()
        const timer = setInterval(pollModel, 15000)
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
        <div className="chat">
            <div className="chat__header">
                <div>
                    <h2 className="chat__title">Neural Gateway</h2>
                    <div className="chat__signal-row">
                        <span className="chat__signal-chip">{modelLabel}</span>
                        <span className="chat__signal-chip">{versionLabel}</span>
                        <select
                            value={inferenceMode}
                            onChange={(e) => setInferenceMode(e.target.value as "standard" | "distributed")}
                            className="chat__signal-chip"
                            style={{ background: 'var(--bg-tertiary)', cursor: 'pointer', outline: 'none' }}
                        >
                            <option value="standard">Standard Mode</option>
                            <option value="distributed">Distributed Mode</option>
                        </select>
                    </div>
                </div>
                <div className="chat__status-group">
                    <div className={`chat__status ${ready ? "chat__status--ok" : "chat__status--pending"}`}>
                        {ready ? "NETWORK ONLINE" : "SYNCING MESH"}
                    </div>
                    <div className="chat__status chat__status--neutral">
                        {opsSummary.active_nodes ?? 0} NODES · {successRate}% RELIABILITY
                    </div>
                </div>
            </div>

            <div className="chat__messages">
                {messages.length === 0 ? (
                    <div className="chat__empty">
                        <div className="chat__empty-icon">S</div>
                        <h3 className="chat__empty-title">Secure Distributed Inference</h3>
                        <p className="chat__empty-hint">
                            Your requests are processed across a trustless P2P mesh. 
                            Select a prompt below or start a conversation.
                        </p>
                        <div className="chat__quick-prompts">
                            {quickPrompts.map((prompt) => (
                                <button
                                    key={prompt}
                                    type="button"
                                    className="chat__quick-btn"
                                    onClick={() => setInput(prompt)}
                                >
                                    {prompt}
                                </button>
                            ))}
                        </div>
                    </div>
                ) : (
                    messages.map((msg, i) => (
                        <div
                            key={i}
                            className={`message ${msg.role === "user" ? "message--user" : "message--assistant"}`}
                        >
                            <div className="message__avatar">
                                {msg.role === "user" ? "USR" : "SHD"}
                            </div>
                            <div className="message__content">
                                <div className="message__bubble">
                                    {msg.content || (
                                        <div className="typing">
                                            <div className="typing__dot" />
                                            <div className="typing__dot" />
                                            <div className="typing__dot" />
                                        </div>
                                    )}
                                </div>
                                <div className="message__meta">
                                    {msg.role === "assistant" ? "shard-hybrid" : "local-node"} · {new Date(msg.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                                </div>
                            </div>
                        </div>
                    ))
                )}
                <div ref={messagesEndRef} />
            </div>

            <div className="chat__input-area">
                <div className="chat__input-wrapper">
                    <textarea
                        ref={textareaRef}
                        className="chat__input"
                        placeholder={mode === "loading" ? "Establishing connection..." : "Ask Shard anything..."}
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
                        aria-label="Send"
                    >
                        ➤
                    </button>
                </div>
                <div className="chat__input-hint">
                    {streaming ? "Network is responding..." : "Press Enter to send, Shift+Enter for new line"}
                </div>
            </div>
        </div>
    )
}
