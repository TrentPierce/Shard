"use client"

import { useEffect, useRef, useState } from "react"
import { useAppContext, type NodeMode } from "@/lib/context"
import { sendMessage, type ChatMessage } from "@/lib/api"
import { useProductSignals } from "@/hooks/useProductSignals"

interface ChatPanelProps {
    mode: NodeMode
}

import { Activity, Cpu, Send, MessageSquare, Sparkles, User, Bot } from "lucide-react"

import { apiUrl } from "@/lib/config"

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

    const modelLabel = topology?.model_id ?? "TinyLlama-1.1B"

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
                // ignore
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
            console.error("Chat Error:", err)
            setMessages((prev) => {
                const updated = [...prev]
                const last = updated[updated.length - 1]
                if (last.role === "assistant") {
                    updated[updated.length - 1] = {
                        ...last,
                        content: "Network error: Failed to connect to the Shard verifier node. Please ensure the daemon is running or check your connection.",
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

    const quickPrompts = [
        "How does distributed inference work?",
        "What is the current network health?",
        "Explain Shard architecture.",
    ]

    return (
        <main className="chat-panel">
            {/* Panel Header */}
            <header className="chat-panel__header">
                <div className="flex items-center gap-md">
                    <div className="avatar-orb avatar-orb--bot">
                        <Sparkles size={18} />
                    </div>
                    <div>
                        <h2 className="text-lg font-bold tracking-tight leading-tight">Neural Assistant</h2>
                        <div className="flex items-center gap-md mt-1">
                            <span className="badge badge-primary flex items-center gap-1">
                                <Cpu size={10} /> {modelLabel}
                            </span>
                            <span className="text-xs text-muted font-medium flex items-center gap-1">
                                <Activity size={10} /> {successRate}% Reliability
                            </span>
                        </div>
                    </div>
                </div>
                
                <div className="flex flex-col items-end">
                    <select
                        value={inferenceMode}
                        onChange={(e) => setInferenceMode(e.target.value as any)}
                        className="mode-selector"
                    >
                        <option value="distributed">Distributed Mesh</option>
                        <option value="standard">Local Direct</option>
                    </select>
                    <div className="flex items-center gap-1.5 mt-1.5">
                        <div className={`w-2 h-2 rounded-full ${ready ? 'bg-primary animate-pulse' : 'bg-error'}`} />
                        <span className="text-[10px] uppercase font-bold tracking-widest text-muted">
                            {ready ? 'Connected' : 'Syncing'}
                        </span>
                    </div>
                </div>
            </header>

            {/* Messages Area */}
            <div className="chat-panel__messages">
                {messages.length === 0 ? (
                    <div className="empty-state">
                        <div className="empty-state__icon">
                            <Bot size={40} className="text-primary" />
                        </div>
                        <h3 className="text-xl font-bold mt-4">How can I help you build?</h3>
                        <p className="text-muted mt-2 max-w-sm">
                            Experience decentralized inference powered by the Shard Network. 
                            Your compute, your network, your AI.
                        </p>
                        
                        {!ready && (
                            <div className="mt-4 p-4 card card-glass flex flex-col items-center">
                                <p className="text-xs text-muted mb-2">Node connection required for distributed inference.</p>
                                <a href="https://github.com/TrentPierce/Shard/releases" target="_blank" className="btn btn-secondary btn-sm">
                                    Download Verifier App
                                </a>
                            </div>
                        )}

                        <div className="quick-prompts-grid mt-8">
                            {quickPrompts.map(p => (
                                <button key={p} onClick={() => setInput(p)} className="quick-prompt-card">
                                    {p}
                                </button>
                            ))}
                        </div>
                    </div>
                ) : (
                    messages.map((msg, i) => (
                        <div key={i} className={`message-row ${msg.role === 'user' ? 'message-row--user' : 'message-row--bot'}`}>
                            <div className="message-bubble">
                                <div className="message-header">
                                    <span className="role-label">{msg.role === 'user' ? 'You' : 'Shard AI'}</span>
                                    <span className="time-label">{new Date(msg.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                                </div>
                                <div className="message-content">
                                    {msg.content || <span className="typing-dots"><span>.</span><span>.</span><span>.</span></span>}
                                </div>
                            </div>
                        </div>
                    ))
                )}
                <div ref={messagesEndRef} />
            </div>

            {/* Input Area */}
            <footer className="chat-panel__input-area">
                <div className="input-wrapper">
                    <textarea
                        ref={textareaRef}
                        placeholder={ready ? "Message Neural Assistant..." : "Establishing network connection..."}
                        value={input}
                        onChange={(e) => setInput(e.target.value)}
                        onKeyDown={handleKeyDown}
                        disabled={streaming}
                        rows={1}
                    />
                    <button 
                        onClick={handleSend}
                        disabled={!input.trim() || streaming}
                        className="send-button"
                    >
                        {streaming ? (
                            <div className="spinner" />
                        ) : (
                            <Send size={18} />
                        )}
                    </button>
                </div>
                <div className="input-footer">
                    <span>{streaming ? 'Generating verified tokens via Shard Mesh...' : 'End-to-end encrypted • Decentralized compute'}</span>
                </div>
            </footer>

            <style jsx>{`
                .chat-panel {
                    display: flex;
                    flex-direction: column;
                    height: 100%;
                    background: transparent;
                }
                .chat-panel__header {
                    padding: var(--space-lg);
                    border-bottom: 1px solid var(--border);
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    background: rgba(255, 255, 255, 0.02);
                }
                .avatar-orb {
                    width: 44px;
                    height: 44px;
                    border-radius: 50%;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    background: var(--primary-glow);
                    color: var(--primary);
                    border: 1px solid rgba(0, 212, 170, 0.2);
                }
                .mode-selector {
                    background: var(--bg-tertiary);
                    border: 1px solid var(--border);
                    color: var(--text-secondary);
                    font-size: 11px;
                    font-weight: 600;
                    padding: 4px 8px;
                    border-radius: var(--radius-sm);
                    cursor: pointer;
                    width: auto;
                }
                .chat-panel__messages {
                    flex: 1;
                    overflow-y: auto;
                    padding: var(--space-xl) var(--space-lg);
                    display: flex;
                    flex-direction: column;
                    gap: var(--space-xl);
                }
                .empty-state {
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    justify-content: center;
                    height: 100%;
                    text-align: center;
                }
                .empty-state__icon {
                    width: 80px;
                    height: 80px;
                    background: var(--bg-tertiary);
                    border-radius: 24px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    border: 1px solid var(--border);
                }
                .quick-prompts-grid {
                    display: grid;
                    grid-template-columns: repeat(2, 1fr);
                    gap: var(--space-sm);
                    width: 100%;
                    max-width: 500px;
                }
                .quick-prompt-card {
                    background: var(--bg-tertiary);
                    border: 1px solid var(--border);
                    padding: var(--space-md);
                    border-radius: var(--radius-md);
                    color: var(--text-secondary);
                    font-size: 13px;
                    text-align: left;
                    cursor: pointer;
                    transition: all var(--transition-fast);
                }
                .quick-prompt-card:hover {
                    border-color: var(--primary);
                    color: var(--text-primary);
                    background: var(--bg-elevated);
                }
                .message-row {
                    display: flex;
                    width: 100%;
                }
                .message-row--user { justify-content: flex-end; }
                .message-row--bot { justify-content: flex-start; }
                
                .message-bubble {
                    max-width: 80%;
                    padding: var(--space-md) var(--space-lg);
                    border-radius: var(--radius-lg);
                    position: relative;
                }
                .message-row--user .message-bubble {
                    background: var(--primary);
                    color: var(--text-inverse);
                    border-bottom-right-radius: 4px;
                }
                .message-row--bot .message-bubble {
                    background: var(--bg-tertiary);
                    color: var(--text-primary);
                    border: 1px solid var(--border);
                    border-bottom-left-radius: 4px;
                }
                .message-header {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    margin-bottom: 4px;
                    font-size: 10px;
                    font-weight: 700;
                    text-transform: uppercase;
                    letter-spacing: 0.05em;
                    opacity: 0.8;
                }
                .message-content {
                    font-size: 15px;
                    line-height: 1.6;
                    white-space: pre-wrap;
                }
                .chat-panel__input-area {
                    padding: var(--space-lg);
                    background: rgba(255, 255, 255, 0.01);
                    border-top: 1px solid var(--border);
                }
                .input-wrapper {
                    background: var(--bg-tertiary);
                    border: 1px solid var(--border-hover);
                    border-radius: var(--radius-lg);
                    display: flex;
                    align-items: flex-end;
                    padding: 8px;
                    transition: border-color var(--transition-fast);
                }
                .input-wrapper:focus-within {
                    border-color: var(--primary);
                }
                textarea {
                    flex: 1;
                    background: transparent;
                    border: none !important;
                    box-shadow: none !important;
                    padding: 8px 12px;
                    color: var(--text-primary);
                    font-size: 15px;
                    resize: none;
                    min-height: 40px;
                }
                .send-button {
                    width: 40px;
                    height: 40px;
                    border-radius: 12px;
                    background: var(--primary);
                    color: var(--text-inverse);
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    cursor: pointer;
                    transition: all var(--transition-fast);
                    border: none;
                    flex-shrink: 0;
                }
                .send-button:hover:not(:disabled) {
                    background: var(--primary-hover);
                    transform: scale(1.05);
                }
                .send-button:disabled {
                    background: var(--bg-elevated);
                    color: var(--text-muted);
                    cursor: not-allowed;
                }
                .input-footer {
                    display: flex;
                    justify-content: center;
                    margin-top: 12px;
                    font-size: 10px;
                    color: var(--text-muted);
                    text-transform: uppercase;
                    letter-spacing: 0.05em;
                    font-weight: 600;
                }
                .spinner {
                    width: 18px;
                    height: 18px;
                    border: 2px solid rgba(255,255,255,0.2);
                    border-top-color: white;
                    border-radius: 50%;
                    animation: spin 0.8s linear infinite;
                }
                @keyframes spin {
                    to { transform: rotate(360deg); }
                }
                .typing-dots {
                    display: flex;
                    gap: 2px;
                }
                .typing-dots span {
                    animation: blink 1.4s infinite both;
                }
                .typing-dots span:nth-child(2) { animation-delay: 0.2s; }
                .typing-dots span:nth-child(3) { animation-delay: 0.4s; }
                @keyframes blink {
                    0% { opacity: 0.2; }
                    20% { opacity: 1; }
                    100% { opacity: 0.2; }
                }
            `}</style>
        </main>
    )
}
