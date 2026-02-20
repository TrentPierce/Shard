"use client"

import { useEffect, useRef, useState } from "react"
import type { NodeMode } from "@/app/page"
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
                            (err?.message ?? "Could not connect to the local Shard daemon. Ensure the background process is running and the model is downloaded."),
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
    const assistantMessages = messages.filter((msg) => msg.role === "assistant").length
    const versionLabel = health.rust_version ? `daemon ${health.rust_version}` : "daemon unknown"
    const uptimeLabel =
        typeof health.rust_uptime_ms === "number" && health.rust_uptime_ms > 0
            ? `${Math.floor(health.rust_uptime_ms / 3600000)}h uptime`
            : "uptime unavailable"
    const lastIncidentLabel =
        health.last_incident && health.last_incident !== "none"
            ? `incident: ${health.last_incident}`
            : "incident-free"

    const quickPrompts = [
        "What can this network do right now?",
        "Explain Scout vs Shard in 2 sentences.",
        "Give me a quick health summary.",
    ]

    return (
        <div className="flex flex-col flex-1 min-h-0 overflow-hidden bg-primary" role="main">
            <div className="px-4 py-4 md:px-8 md:py-6 border-b border-glass-border bg-glass-bg/50 backdrop-blur-md flex flex-col md:flex-row justify-between items-start md:items-center gap-4 shrink-0">
                <div className="flex flex-col gap-1">
                    <h2 className="text-lg font-display font-bold text-white tracking-tight">Intelligence Gateway</h2>
                    <div className="flex flex-wrap items-center gap-2 md:gap-3">
                        <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-accent-cyan/10 border border-accent-cyan/20 text-[10px] text-accent-cyan font-mono uppercase tracking-wider">
                            {modelLabel}
                        </div>
                        <div className="h-1 w-1 rounded-full bg-muted"></div>
                        <div className="text-[10px] text-muted font-medium uppercase tracking-wider">{versionLabel}</div>
                        <div className="h-1 w-1 rounded-full bg-muted"></div>
                        <select
                            value={inferenceMode}
                            onChange={(e) => setInferenceMode(e.target.value as "standard" | "distributed")}
                            className="text-[10px] bg-transparent border border-glass-border rounded px-2 py-1 text-muted uppercase"
                            aria-label="Inference mode"
                        >
                            <option value="standard">Standard inference</option>
                            <option value="distributed">Shard distributed</option>
                        </select>
                    </div>
                </div>
                <div className="w-full md:w-auto flex items-center justify-between md:justify-end gap-4">
                    <div className="flex flex-col items-start md:items-end gap-1">
                        <div className={`text-[10px] font-bold uppercase tracking-widest ${ready ? "text-accent-emerald" : "text-accent-rose"}`}>
                            {ready ? "● Sync Active" : "○ Disconnected"}
                        </div>
                        <div className="text-[9px] text-muted uppercase tracking-tighter tabular-nums">
                            {analytics.sessions} sessions · {successRate}% reliability
                        </div>
                        <div className="text-[9px] text-muted uppercase tracking-tighter tabular-nums">
                            {opsSummary.active_nodes ?? 0} nodes · p95 {Math.round(opsSummary.p95_latency_ms ?? 0)}ms
                        </div>
                        <div className="text-[9px] text-muted uppercase tracking-tighter tabular-nums">
                            offload {Math.round(opsSummary.offload_percentage_estimate ?? 0)}% · savings {Math.round(opsSummary.estimated_gpu_savings_percent ?? 0)}%
                        </div>
                    </div>
                </div>
            </div>

            <div className="flex-1 overflow-y-auto px-4 py-5 md:px-8 md:py-8 flex flex-col gap-4 md:gap-6 min-h-0" role="log">
                {messages.length === 0 ? (
                    <div className="flex-1 flex flex-col items-center justify-center gap-8 max-w-xl mx-auto text-center opacity-80 scale-95 transition-all duration-700">
                        <div className="w-20 h-20 rounded-3xl bg-gradient-to-br from-accent-cyan to-accent-blue flex items-center justify-center text-3xl font-bold text-white shadow-glow-cyan overflow-hidden relative">
                            <div className="absolute inset-0 bg-[url('https://www.transparenttextures.com/patterns/carbon-fibre.png')] opacity-20"></div>
                            S
                        </div>
                        <div className="flex flex-col gap-3">
                            <h3 className="text-2xl font-display font-extrabold text-white">How can Shard help you?</h3>
                            <p className="text-secondary text-sm leading-relaxed px-4">
                                Experience trustless distributed inference. Your requests are parallel-vetted across the network for cryptographic correctness and output integrity.
                            </p>
                        </div>
                        <div className="flex flex-wrap justify-center gap-3">
                            {quickPrompts.map((prompt) => (
                                <button
                                    key={prompt}
                                    type="button"
                                    className="px-4 py-2 rounded-xl bg-tertiary border border-glass-border text-xs text-secondary hover:text-white hover:border-accent-cyan/40 hover:bg-tertiary/80 transition-smooth"
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
                            className={`flex gap-3 md:gap-5 max-w-full md:max-w-4xl ${msg.role === "user" ? "ml-auto flex-row-reverse" : "mr-auto"}`}
                        >
                            <div className={`w-8 h-8 md:w-10 md:h-10 rounded-2xl shrink-0 flex items-center justify-center text-[9px] md:text-[10px] font-bold uppercase tracking-tighter
                                ${msg.role === "user"
                                    ? "bg-tertiary border border-glass-border text-muted"
                                    : "bg-accent-cyan/10 border border-accent-cyan/20 text-accent-cyan"}`}
                            >
                                {msg.role === "user" ? "USR" : "SHD"}
                            </div>
                            <div className={`flex flex-col gap-2 ${msg.role === "user" ? "items-end text-right" : "items-start text-left"}`}>
                                <div className={`px-4 py-3 md:px-5 md:py-4 rounded-2xl text-sm leading-relaxed shadow-sm max-w-[85vw] md:max-w-3xl break-words
                                    ${msg.role === "user"
                                        ? "bg-secondary text-primary border border-glass-border rounded-tr-none"
                                        : "bg-tertiary/50 text-primary border border-glass-border rounded-tl-none"}`}
                                >
                                    {msg.content || (
                                        <div className="flex gap-1.5 py-1.5">
                                            <div className="w-1.5 h-1.5 rounded-full bg-accent-cyan animate-bounce [animation-delay:-0.3s]" />
                                            <div className="w-1.5 h-1.5 rounded-full bg-accent-cyan animate-bounce [animation-delay:-0.15s]" />
                                            <div className="w-1.5 h-1.5 rounded-full bg-accent-cyan animate-bounce" />
                                        </div>
                                    )}
                                </div>
                                <div className="text-[10px] text-muted font-mono uppercase opacity-60">
                                    {msg.role === "assistant" ? "shard-hybrid-v1" : "local-auth-node"} · {new Date(msg.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                                </div>
                            </div>
                        </div>
                    ))
                )}
                <div ref={messagesEndRef} />
            </div>

            <div className="px-4 pb-4 md:px-8 md:pb-8 pt-2 bg-gradient-to-t from-primary via-primary to-transparent">
                <div className="relative group max-w-4xl mx-auto">
                    <div className="absolute -inset-0.5 bg-gradient-to-r from-accent-cyan to-accent-blue rounded-3xl blur opacity-10 group-focus-within:opacity-30 transition-smooth"></div>
                    <div className="relative flex flex-col bg-secondary border border-glass-border rounded-2xl overflow-hidden shadow-2xl">
                        <textarea
                            ref={textareaRef}
                            className="w-full bg-transparent px-4 py-4 md:px-6 md:py-5 text-sm text-primary placeholder:text-muted focus:outline-none resize-none min-h-[56px]"
                            id="chat-input"
                            name="chat-input"
                            placeholder={mode === "loading" ? "Establishing secure channel..." : "Message the Shard network..."}
                            value={input}
                            onChange={(e) => setInput(e.target.value)}
                            onKeyDown={handleKeyDown}
                            disabled={mode === "loading"}
                            rows={1}
                            aria-label="Type your message here"
                        />
                        <div className="px-4 py-3 md:px-6 border-t border-glass-border bg-tertiary/30 flex justify-between items-center gap-3">
                            <span className="text-[10px] text-muted font-medium uppercase tracking-widest">
                                Verification active · {input.length} chars
                            </span>
                            <button
                                className="px-4 py-1.5 rounded-lg bg-accent-cyan text-primary text-[11px] font-bold uppercase tracking-widest hover:brightness-110 active:scale-95 disabled:opacity-30 disabled:grayscale transition-smooth"
                                onClick={handleSend}
                                disabled={!input.trim() || streaming || mode === "loading"}
                                title="Send message"
                                type="submit"
                            >
                                Dispatch
                            </button>
                        </div>
                    </div>
                </div>
                <p className="text-center mt-4 text-[9px] text-muted uppercase tracking-[0.2em] opacity-40">
                    Trust but verify · Shard Distributed v0.4.9
                </p>
            </div>
        </div>
    )
}

