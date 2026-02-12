"use client"

import { useEffect, useRef, useState } from "react"
import type { NodeMode } from "@/app/page"
import { sendMessage, type ChatMessage } from "@/lib/api"

interface ChatPanelProps {
    mode: NodeMode
}

export default function ChatPanel({ mode }: ChatPanelProps) {
    const [messages, setMessages] = useState<ChatMessage[]>([])
    const [input, setInput] = useState("")
    const [streaming, setStreaming] = useState(false)
    const messagesEndRef = useRef<HTMLDivElement>(null)
    const textareaRef = useRef<HTMLTextAreaElement>(null)

    const scrollToBottom = () => {
        messagesEndRef.current?.scrollIntoView({ behavior: "smooth" })
    }

    useEffect(() => {
        scrollToBottom()
    }, [messages])

    // Auto-resize textarea
    useEffect(() => {
        const el = textareaRef.current
        if (el) {
            el.style.height = "24px"
            el.style.height = Math.min(el.scrollHeight, 120) + "px"
        }
    }, [input])

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

        // Create placeholder assistant message
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
                    // Update the last message (assistant) with streaming content
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
                }
            )
        } catch (err: any) {
            setMessages((prev) => {
                const updated = [...prev]
                const last = updated[updated.length - 1]
                if (last.role === "assistant") {
                    updated[updated.length - 1] = {
                        ...last,
                        content:
                            "⚠ Connection error: " +
                            (err?.message ?? "Could not reach the Oracle API. Is the Python server running on :8000?"),
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

    return (
        <div className="chat" role="main" aria-label="Chat interface">
            {/* ── Messages ── */}
            <div className="chat__messages" role="log" aria-live="polite" aria-atomic="false" aria-label="Chat messages">
                {messages.length === 0 ? (
                    <div className="chat__empty animate-slide-up">
                        <div className="chat__empty-icon">⬡</div>
                        <div className="chat__empty-title">Welcome to Shard</div>
                        <div className="chat__empty-hint">
                            Ask anything. Your prompt will be processed through the
                            decentralized inference mesh — Scout peers generate draft tokens,
                            Oracles verify them.
                        </div>
                    </div>
                ) : (
                    messages.map((msg, i) => (
                        <div
                            key={i}
                            className={`message message--${msg.role}`}
                            role="article"
                            aria-labelledby={`msg-sender-${i}`}
                        >
                            <div className="message__avatar" id={`msg-sender-${i}`} aria-hidden="true">
                                {msg.role === "user" ? "U" : "S"}
                            </div>
                            <div>
                                <div className="message__bubble" role="alert" aria-live="off">
                                    {msg.content || (
                                        <div className="typing">
                                            <div className="typing__dot" />
                                            <div className="typing__dot" />
                                            <div className="typing__dot" />
                                        </div>
                                    )}
                                </div>
                                <div className="message__meta">
                                    {msg.role === "assistant" ? "shard-hybrid" : "you"} ·{" "}
                                    {new Date(msg.timestamp).toLocaleTimeString()}
                                </div>
                            </div>
                        </div>
                    ))
                )}
                <div ref={messagesEndRef} />
            </div>

            {/* ── Input ── */}
            <div className="chat__input-area">
                <div className="chat__input-wrapper">
                    <textarea
                        ref={textareaRef}
                        className="chat__input"
                        id="chat-input"
                        name="chat-input"
                        placeholder={
                            mode === "loading"
                                ? "Connecting to network…"
                                : "Send a message to the Shard network…"
                        }
                        value={input}
                        onChange={(e) => setInput(e.target.value)}
                        onKeyDown={handleKeyDown}
                        disabled={mode === "loading"}
                        rows={1}
                        aria-label="Type your message here"
                        aria-describedby="chat-hint"
                    />
                    <button
                        className="chat__send-btn"
                        onClick={handleSend}
                        disabled={!input.trim() || streaming || mode === "loading"}
                        title="Send message"
                        type="submit"
                        aria-label="Send message"
                    >
                        <span aria-hidden="true">↑</span>
                    </button>
                </div>
            </div>
        </div>
    )
}
