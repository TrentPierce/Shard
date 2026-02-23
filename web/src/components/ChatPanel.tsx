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
        <main className={`chat ${isEnterprise ? 'chat--enterprise' : 'chat--terminal'}`} aria-label="Shard gateway">
            <div className="chat__header glass-card">
                <div className="flex items-center gap-md">
                    {isEnterprise && (
                        <div className="w-10 h-10 rounded-full bg-primary-glow flex items-center justify-center text-primary">
                            <Sparkles size={20} />
                        </div>
                    )}
                    <div>
                        <h2 className="chat__title">
                            {isEnterprise ? "Neural Assistant" : "[ NEURAL_GATEWAY_V1.4 ]"}
                        </h2>
                        <div className="chat__meta">
                            <span className="flex items-center gap-1"><Cpu size={12} /> {modelLabel.toUpperCase()}</span>
                            <span className="flex items-center gap-1"><Activity size={12} /> {versionLabel.toUpperCase()}</span>
                            <select
                                id="inference-mode-select"
                                value={inferenceMode}
                                onChange={(e) => setInferenceMode(e.target.value as "standard" | "distributed")}
                                className="chat__mode-select"
                            >
                                <option value="standard">{isEnterprise ? "Standard Mode" : "MODE: SINGLE_NODE"}</option>
                                <option value="distributed">{isEnterprise ? "Distributed Mesh" : "MODE: DIST_MESH"}</option>
                            </select>
                        </div>
                    </div>
                </div>
                <div className="text-right">
                    <div className={`chat__status ${ready ? "text-primary" : "text-error"}`}>
                        {ready ? (isEnterprise ? "CONNECTED" : "STATUS: ONLINE") : (isEnterprise ? "SYNCING..." : "STATUS: SYNCING")}
                    </div>
                    <div className="chat__stats">
                        {opsSummary.active_nodes ?? 0} NODES • {successRate}% RELIABILITY
                    </div>
                </div>
            </div>

            <div className="chat__messages">
                {messages.length === 0 ? (
                    <div className="chat__empty">
                        {!isEnterprise ? (
                            <pre className="chat__ascii">{`
   _____ _    _          _____  _____  
  / ____| |  | |   /\   |  __ \\|  __ \\ 
 | (___ | |__| |  /  \\  | |__) | |  | |
  \\___ \\|  __  | / /\\ \\ |  _  /| |  | |
  ____) | |  | |/ ____ \\| | \\ \\| |__| |
 |_____/|_|  |_/_/    \\_\\_|  \\_\\_____/ 
                                       
        `}</pre>
                        ) : (
                            <div className="w-16 h-16 rounded-2xl bg-primary-glow flex items-center justify-center text-primary mb-6">
                                <MessageSquare size={32} />
                            </div>
                        )}
                        <h3 className="chat__empty-title">
                            {isEnterprise ? "What can I help you build?" : "ESTABLISHING_TRUSTLESS_MESH_LINK..."}
                        </h3>
                        <p className="chat__empty-hint">
                            {isEnterprise
                                ? "Ask anything. All compute is distributed across the Shard decentralized network for private, local-first inference."
                                : "// ALL COMPUTE IS DECENTRALIZED AND ENCRYPTED"}
                        </p>
                        <div className="chat__quick-prompts">
                            {quickPrompts.map((prompt) => (
                                <button
                                    key={prompt}
                                    type="button"
                                    className="chat__quick-prompt-btn"
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
                        >
                            <div className="message__avatar">
                                <span className="message__role">
                                    {isEnterprise
                                        ? (msg.role === "user" ? "YOU" : "SHARD AI")
                                        : (msg.role === "user" ? "GUEST@SHARD-NET:~$" : "SYSTEM@SHARD-CORE:~#")}
                                </span>
                                <span className="message__separator">•</span>
                                <span className="message__time">{new Date(msg.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                            </div>
                            <div className="message__bubble">
                                {msg.content || (
                                    <span className="animate-pulse">●</span>
                                )}
                                {streaming && i === messages.length - 1 && msg.role === 'assistant' && (
                                    <span className="message__cursor">●</span>
                                )}
                            </div>
                        </div>
                    ))
                )}
                <div ref={messagesEndRef} />
            </div>

            <div className="chat__input-area">
                <div className="chat__input-wrapper">
                    {!isEnterprise && <span className="chat__prompt-char">&gt;</span>}
                    <textarea
                        id="chat-prompt-input"
                        ref={textareaRef}
                        className="chat__input"
                        placeholder={mode === "loading" ? "Initializing..." : (isEnterprise ? "Ask anything..." : "COMMAND:")}
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
                        {streaming ? <div className="animate-spin h-4 w-4 border-2 border-white/20 border-t-white rounded-full" /> : (isEnterprise ? <Send size={18} /> : "[ SEND ]")}
                    </button>
                </div>
                <div className="chat__footer-hint">
                    {streaming ? (isEnterprise ? "Assistant is thinking..." : "WAITING_FOR_SWARM_RESPONSE...") : (isEnterprise ? "Private & Decentralized" : "READY_FOR_INPUT // SHIFT+ENTER_NEWLINE")}
                </div>
            </div>

            <style jsx>{`
                .chat {
                    display: flex;
                    flex-direction: column;
                    height: 100%;
                    background: var(--bg-primary);
                    color: var(--text-primary);
                    position: relative;
                }
                .chat__header {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    padding: var(--space-md) var(--space-lg);
                    border-bottom: 1px solid var(--border);
                    background: var(--bg-secondary);
                }
                .chat__title {
                    font-size: 1.125rem;
                    font-weight: 700;
                    letter-spacing: 0.05em;
                }
                .chat--terminal .chat__title {
                    font-family: var(--font-mono);
                    letter-spacing: 2px;
                }
                .chat__meta {
                    display: flex;
                    gap: var(--space-md);
                    margin-top: var(--space-xs);
                    font-size: 0.6875rem;
                    opacity: 0.7;
                    font-family: var(--font-mono);
                }
                .chat__mode-select {
                    background: transparent;
                    color: inherit;
                    border: none;
                    border-bottom: 1px solid var(--border);
                    padding: 0;
                    font-size: inherit;
                    cursor: pointer;
                    width: auto;
                    border-radius: 0;
                }
                .chat__status {
                    font-size: 0.75rem;
                    font-weight: 800;
                    font-family: var(--font-mono);
                }
                .chat__stats {
                    font-size: 0.625rem;
                    opacity: 0.6;
                    margin-top: 2px;
                    font-family: var(--font-mono);
                }
                .chat__messages {
                    flex: 1;
                    overflow-y: auto;
                    padding: var(--space-xl);
                    display: flex;
                    flex-direction: column;
                }
                .chat--terminal .chat__messages {
                    font-family: var(--font-mono);
                }
                .chat__empty {
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    justify-content: center;
                    height: 100%;
                    text-align: center;
                }
                .chat__ascii {
                    color: var(--primary);
                    margin-bottom: var(--space-lg);
                    font-size: 0.75rem;
                    line-height: 1.2;
                }
                .chat__empty-title {
                    font-size: 1.5rem;
                    font-weight: 700;
                    margin-bottom: var(--space-sm);
                }
                .chat--terminal .chat__empty-title {
                    font-size: 0.875rem;
                    font-family: var(--font-mono);
                }
                .chat__empty-hint {
                    font-size: 0.875rem;
                    color: var(--text-muted);
                    max-width: 400px;
                }
                .chat__quick-prompts {
                    margin-top: var(--space-2xl);
                    display: flex;
                    flex-wrap: wrap;
                    justify-content: center;
                    gap: var(--space-sm);
                }
                .chat__quick-prompt-btn {
                    background: var(--bg-tertiary);
                    border: 1px solid var(--border);
                    color: var(--text-secondary);
                    padding: var(--space-sm) var(--space-md);
                    border-radius: var(--radius-md);
                    cursor: pointer;
                    font-size: 0.75rem;
                    transition: all var(--transition-fast);
                }
                .chat__quick-prompt-btn:hover {
                    border-color: var(--primary);
                    color: var(--text-primary);
                    background: var(--bg-elevated);
                }
                .chat--enterprise .chat__quick-prompt-btn {
                    border-radius: var(--radius-full);
                    background: var(--glass-bg);
                }
                .message {
                    margin-bottom: var(--space-xl);
                    max-width: 85%;
                }
                .chat--terminal .message {
                    max-width: 100%;
                }
                .message--user {
                    align-self: flex-end;
                }
                .message--assistant {
                    align-self: flex-start;
                }
                .message__avatar {
                    font-size: 0.6875rem;
                    margin-bottom: var(--space-xs);
                    display: flex;
                    gap: var(--space-sm);
                    align-items: center;
                    font-family: var(--font-mono);
                }
                .message--user .message__avatar {
                    flex-direction: row-reverse;
                }
                .message__role {
                    font-weight: 700;
                }
                .message--user .message__role { color: var(--secondary); }
                .message--assistant .message__role { color: var(--primary); }
                
                .message__bubble {
                    padding: var(--space-md) var(--space-lg);
                    border-radius: var(--radius-lg);
                    font-size: 0.9375rem;
                    line-height: 1.6;
                    position: relative;
                }
                .message--user .message__bubble {
                    background: var(--bg-tertiary);
                    color: var(--text-primary);
                    border: 1px solid var(--border);
                    border-bottom-right-radius: var(--radius-xs);
                }
                .message--assistant .message__bubble {
                    background: var(--bg-secondary);
                    color: var(--text-primary);
                    border: 1px solid var(--border);
                    border-bottom-left-radius: var(--radius-xs);
                }
                .chat--enterprise .message--user .message__bubble {
                    background: var(--primary-glow);
                    border-color: var(--primary);
                }
                .chat--terminal .message__bubble {
                    border-radius: 0;
                    border: none;
                    padding: 0 var(--space-sm);
                    background: transparent !important;
                }
                .chat--terminal .message--user .message__bubble {
                    border-left: 2px solid var(--secondary);
                }
                .chat--terminal .message--assistant .message__bubble {
                    border-left: 2px solid var(--primary);
                }
                .message__cursor {
                    display: inline-block;
                    width: 8px;
                    height: 15px;
                    background: var(--primary);
                    margin-left: 4px;
                    animation: pulse 1s infinite;
                    vertical-align: middle;
                }
                .chat__input-area {
                    padding: var(--space-lg);
                    border-top: 1px solid var(--border);
                    background: var(--bg-secondary);
                }
                .chat__input-wrapper {
                    display: flex;
                    align-items: center;
                    background: var(--bg-tertiary);
                    border: 1px solid var(--border);
                    border-radius: var(--radius-lg);
                    padding: var(--space-xs) var(--space-sm);
                    position: relative;
                }
                .chat--enterprise .chat__input-wrapper {
                    border-radius: var(--radius-xl);
                }
                .chat--terminal .chat__input-wrapper {
                    border-radius: 0;
                    background: transparent;
                }
                .chat__prompt-char {
                    color: var(--primary);
                    font-family: var(--font-mono);
                    margin-left: var(--space-sm);
                    font-weight: 700;
                }
                .chat__input {
                    background: transparent;
                    border: none;
                    box-shadow: none;
                    padding: var(--space-sm) var(--space-md);
                    font-size: 0.9375rem;
                    min-height: 44px;
                }
                .chat__input:focus {
                    box-shadow: none;
                }
                .chat__send-btn {
                    background: var(--primary);
                    color: var(--text-inverse);
                    border: none;
                    border-radius: var(--radius-md);
                    padding: var(--space-sm) var(--space-lg);
                    font-weight: 700;
                    cursor: pointer;
                    transition: all var(--transition-fast);
                    display: flex;
                    align-items: center;
                    justify-content: center;
                }
                .chat__send-btn:hover:not(:disabled) {
                    background: var(--primary-hover);
                    transform: translateY(-1px);
                }
                .chat__send-btn:disabled {
                    opacity: 0.5;
                    cursor: not-allowed;
                    background: var(--text-muted);
                }
                .chat--terminal .chat__send-btn {
                    background: transparent;
                    color: var(--primary);
                    font-family: var(--font-mono);
                    border: 1px solid var(--primary);
                }
                .chat--terminal .chat__send-btn:hover:not(:disabled) {
                    background: var(--primary-glow);
                }
                .chat__footer-hint {
                    font-size: 0.625rem;
                    color: var(--text-muted);
                    margin-top: var(--space-sm);
                    text-transform: uppercase;
                    letter-spacing: 1px;
                    text-align: center;
                    font-family: var(--font-mono);
                }
            `}</style>
        </main>
    )

}
