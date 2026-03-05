/**
 * Shard API Client
 *
 * Handles communication with the Python Shard API (port 8000).
 * Supports both streaming (SSE) and non-streaming chat completions.
 */

import { apiUrl } from "./config"
import { DEFAULT_MODEL_ID } from "./model"
import { canUseLocalDaemonFallback, localDaemonUrl } from "./runtime"

// ─── Types ──────────────────────────────────────────────────────────────────

export interface ChatMessage {
    role: "user" | "assistant" | "system"
    content: string
    timestamp: number
}

export interface ChatCompletionRequest {
    model?: string
    messages: { role: string; content: string }[]
    temperature?: number
    max_tokens?: number
    stream?: boolean
}

const API_KEY = process.env.NEXT_PUBLIC_SHARD_API_KEY?.trim() || ""

function sanitizeAssistantContent(raw: string): string {
    let out = ""
    let i = 0
    while (i < raw.length) {
        if (raw[i] === "<" && raw[i + 1] === "|") {
            const end = raw.indexOf("|>", i + 2)
            if (end === -1) break
            i = end + 2
            continue
        }
        out += raw[i]
        i += 1
    }
    return out
}

function authHeaders(): Record<string, string> {
    return API_KEY ? { "Authorization": `Bearer ${API_KEY}` } : {}
}

async function fetchWithLocalFallback(path: string, init: RequestInit): Promise<Response> {
    const primary = apiUrl(path)
    let primaryError: unknown = null
    try {
        const res = await fetch(primary, init)
        if (res.ok || !canUseLocalDaemonFallback()) return res
    } catch {
        primaryError = new Error(`Primary API fetch failed for ${path}`)
    }

    if (!canUseLocalDaemonFallback()) {
        if (primaryError) throw primaryError
        throw new Error(`Primary API failed and local fallback disabled for ${path}`)
    }

    return fetch(localDaemonUrl(path), init)
}

async function sendMessageNonStreaming(
    history: ChatMessage[],
    onToken: (token: string) => void
): Promise<void> {
    const body: ChatCompletionRequest = {
        model: DEFAULT_MODEL_ID,
        messages: history.map((m) => ({ role: m.role, content: m.content })),
        stream: false,
        max_tokens: 256,
    }

    const res = await fetchWithLocalFallback("/v1/chat/completions", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            ...authHeaders(),
        },
        body: JSON.stringify(body),
    })

    if (!res.ok) {
        throw new Error(`API error: ${res.status} ${res.statusText}`)
    }

    const data = await res.json()
    const content = sanitizeAssistantContent(data?.choices?.[0]?.message?.content ?? "")
    if (content) onToken(content)
}


// ─── Streaming Chat ─────────────────────────────────────────────────────────

/**
 * Send a chat message and stream the response via SSE.
 *
 * @param history  - Full conversation history
 * @param onToken  - Called with each streamed token
 * @param onDone   - Called when the stream completes
 */
export async function sendMessage(
    history: ChatMessage[],
    onToken: (token: string) => void,
    onDone: () => void,
    inferenceMode: "standard" | "distributed" = "distributed"
): Promise<void> {
    const startedAt = performance.now()

    const body: ChatCompletionRequest = {
        model: DEFAULT_MODEL_ID,
        messages: history.map((m) => ({ role: m.role, content: m.content })),
        stream: true,
        max_tokens: 256,
    }

    const res = await fetchWithLocalFallback("/v1/chat/completions", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "X-Shard-Inference-Mode": inferenceMode,
            ...authHeaders(),
        },
        body: JSON.stringify(body),
    })

    if (!res.ok) {
        throw new Error(`API error: ${res.status} ${res.statusText}`)
    }

    const reader = res.body?.getReader()
    if (!reader) {
        await sendMessageNonStreaming(history, onToken)
        if (typeof window !== "undefined") {
            window.dispatchEvent(
                new CustomEvent("shard:chat-success", {
                    detail: { latencyMs: Math.round(performance.now() - startedAt) },
                })
            )
        }
        onDone()
        return
    }

    const decoder = new TextDecoder()
    let buffer = ""

    while (true) {
        const { done, value } = await reader.read()
        if (done) break

        buffer += decoder.decode(value, { stream: true })

        // Process SSE lines
        const lines = buffer.split("\n")
        buffer = lines.pop() ?? "" // Keep incomplete line in buffer

        for (const line of lines) {
            const trimmed = line.trim()
            if (!trimmed || !trimmed.startsWith("data: ")) continue

            const data = trimmed.slice(6) // Remove "data: " prefix
            if (data === "[DONE]") {
                if (typeof window !== "undefined") {
                    window.dispatchEvent(
                        new CustomEvent("shard:chat-success", {
                            detail: { latencyMs: Math.round(performance.now() - startedAt) },
                        })
                    )
                }
                onDone()
                return
            }

            try {
                const parsed = JSON.parse(data)
                const delta = parsed?.choices?.[0]?.delta?.content
                if (delta) {
                    const clean = sanitizeAssistantContent(delta)
                    if (clean) onToken(clean)
                }
            } catch {
                // Skip malformed JSON chunks
            }
        }
    }

    if (typeof window !== "undefined") {
        window.dispatchEvent(
            new CustomEvent("shard:chat-success", {
                detail: { latencyMs: Math.round(performance.now() - startedAt) },
            })
        )
    }
    onDone()
}

// ─── Non-Streaming Chat ─────────────────────────────────────────────────────

/**
 * Send a chat message and get the full response at once.
 */
export async function sendMessageSync(
    history: ChatMessage[]
): Promise<string> {
    const body: ChatCompletionRequest = {
        model: DEFAULT_MODEL_ID,
        messages: history.map((m) => ({ role: m.role, content: m.content })),
        stream: false,
        max_tokens: 256,
    }

    const res = await fetchWithLocalFallback("/v1/chat/completions", {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            ...authHeaders(),
        },
        body: JSON.stringify(body),
    })

    if (!res.ok) {
        throw new Error(`API error: ${res.status} ${res.statusText}`)
    }

    const data = await res.json()
    return sanitizeAssistantContent(data?.choices?.[0]?.message?.content ?? "")
}

// ─── Health ─────────────────────────────────────────────────────────────────

/**
 * Check the health of the Shard API.
 */
export async function checkHealth(): Promise<{
    ok: boolean
    rustSidecar: string
    bitnetLoaded: boolean
}> {
    try {
        const res = await fetchWithLocalFallback("/health", {
            method: "GET",
        })
        if (!res.ok) return { ok: false, rustSidecar: "unreachable", bitnetLoaded: false }
        const data = await res.json()
        return {
            ok: data.status === "ok",
            rustSidecar: data.rust_sidecar ?? "unknown",
            bitnetLoaded: data.bitnet_loaded ?? false,
        }
    } catch {
        return { ok: false, rustSidecar: "unreachable", bitnetLoaded: false }
    }
}

