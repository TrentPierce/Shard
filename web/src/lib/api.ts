/**
 * Shard Oracle API Client
 *
 * Handles communication with the Python Oracle API (port 8000).
 * Supports both streaming (SSE) and non-streaming chat completions.
 */

import { apiUrl } from "./config"

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
    onDone: () => void
): Promise<void> {
    const body: ChatCompletionRequest = {
        model: "shard-hybrid",
        messages: history.map((m) => ({ role: m.role, content: m.content })),
        stream: true,
        max_tokens: 256,
    }

        const res = await fetch(apiUrl("/v1/chat/completions"), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
    })

    if (!res.ok) {
        throw new Error(`API error: ${res.status} ${res.statusText}`)
    }

    const reader = res.body?.getReader()
    if (!reader) {
        throw new Error("ReadableStream not supported")
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
                onDone()
                return
            }

            try {
                const parsed = JSON.parse(data)
                const delta = parsed?.choices?.[0]?.delta?.content
                if (delta) {
                    onToken(delta)
                }
            } catch {
                // Skip malformed JSON chunks
            }
        }
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
        model: "shard-hybrid",
        messages: history.map((m) => ({ role: m.role, content: m.content })),
        stream: false,
        max_tokens: 256,
    }

        const res = await fetch(apiUrl("/v1/chat/completions"), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
    })

    if (!res.ok) {
        throw new Error(`API error: ${res.status} ${res.statusText}`)
    }

    const data = await res.json()
    return data?.choices?.[0]?.message?.content ?? ""
}

// ─── Health ─────────────────────────────────────────────────────────────────

/**
 * Check the health of the Oracle API.
 */
export async function checkHealth(): Promise<{
    ok: boolean
    rustSidecar: string
    bitnetLoaded: boolean
}> {
    try {
        const res = await fetch(apiUrl("/health"))
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
