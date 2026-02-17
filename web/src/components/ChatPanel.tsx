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
            )
        } catch (err: any) {
            setMessages((prev) => {
                const updated = [...prev]
                const last = updated[updated.length - 1]
                if (last.role === "assistant") {
                    updated[updated.length - 1] = {
                        ...last,
                        content:
                            "Connection error: " +
                            (err?.message ?? "Could not reach the Shard API at port 8000."),
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

    return (
        <div className="chat" role="main" aria-label="Chat interface">
            <div className="chat__header">
                <div>
                    <h2 className="chat__title">Conversation</h2>
                    <p className="chat__subtitle">OpenAI-compatible chat routed through Shard verification.</p>
                </div>
                <div className="chat__status-group">
                    <span className={`chat__status ${ready ? "chat__status--ok" : "chat__status--pending"}`}>
                        {ready ? "Network Ready" : "Connecting"}
                    </span>
                    <span className="chat__status chat__status--neutral">{assistantMessages} replies</span>
                </div>
            </div>

            <div className="chat__messages" role="log" aria-live="polite" aria-atomic="false" aria-label="Chat messages">
                {messages.length === 0 ? (
                    <div className="chat__empty animate-slide-up">
                        <div className="chat__empty-icon">S</div>
                        <div className="chat__empty-title">Start a request</div>
                        <div className="chat__empty-hint">
                            Messages run through the verification pipeline. Scouts can draft tokens and
                            Shards validate final output.
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
                                {msg.role === "user" ? "You" : "Shard"}
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
                                    {msg.role === "assistant" ? "shard-hybrid" : "you"} · {new Date(msg.timestamp).toLocaleTimeString()}
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
                        id="chat-input"
                        name="chat-input"
                        placeholder={mode === "loading" ? "Connecting to network..." : "Ask a question..."}
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
                        <span aria-hidden="true">^</span>
                    </button>
                </div>
                <p className="chat__input-hint">Enter to send · Shift+Enter for newline</p>
            </div>
        </div>
    )
}
