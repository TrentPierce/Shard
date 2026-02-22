/**
 * Shard Node.js SDK — fetch-based client for distributed inference.
 *
 * Works in Node 18+, Deno, Bun, and browsers (via native fetch).
 * OpenAI-compatible drop-in interface.
 */

import type {
    ChatCompletionRequest,
    ChatCompletionResponse,
    ChatMessage,
    ShardClientOptions,
    StreamChunk,
} from './types.js'
import {
    ShardAPIError,
    ShardAuthError,
    ShardConnectionError,
    ShardTimeoutError,
} from './errors.js'

const DEFAULT_BASE_URL = 'http://localhost:8080'
const DEFAULT_TIMEOUT = 30_000
const DEFAULT_MAX_RETRIES = 3
const RETRY_BACKOFF_BASE = 500

export class ShardClient {
    private readonly apiKey: string | undefined
    private readonly baseUrl: string
    private readonly timeout: number
    private readonly maxRetries: number

    /** Namespaced API for OpenAI compatibility: `client.chat.completions.create(...)` */
    public readonly chat = {
        completions: {
            create: <P extends ChatCompletionRequest>(
                params: P,
            ): Promise<P['stream'] extends true ? AsyncGenerator<StreamChunk, void, unknown> : ChatCompletionResponse> => {
                if (params.stream) {
                    return this._streamingChat(params) as any
                }
                return this._nonStreamingChat(params) as any
            },
        },
    }

    constructor(options: ShardClientOptions = {}) {
        this.apiKey = options.apiKey
        this.baseUrl = (options.baseUrl ?? DEFAULT_BASE_URL).replace(/\/$/, '')
        this.timeout = options.timeout ?? DEFAULT_TIMEOUT
        this.maxRetries = options.maxRetries ?? DEFAULT_MAX_RETRIES
    }

    /**
     * Simple chat helper — send a string or messages, get a response.
     */
    async complete(
        messages: string | ChatMessage[],
        options: Partial<ChatCompletionRequest> = {},
    ): Promise<ChatCompletionResponse> {
        const chatMessages: ChatMessage[] =
            typeof messages === 'string'
                ? [{ role: 'user', content: messages }]
                : messages

        return this._nonStreamingChat({ messages: chatMessages, ...options })
    }

    // ─── Internal ─────────────────────────────────────────────────────

    private async _nonStreamingChat(
        params: ChatCompletionRequest,
    ): Promise<ChatCompletionResponse> {
        const body: ChatCompletionRequest = {
            model: 'default',
            temperature: 0.7,
            sensitive: false,
            top_p: 1.0,
            ...params,
            stream: false,
        }

        for (let attempt = 0; attempt < this.maxRetries; attempt++) {
            try {
                const response = await this._fetch('/v1/chat/completions', {
                    method: 'POST',
                    body: JSON.stringify(body),
                    headers: this._headers(body),
                })

                if (response.status === 401) throw new ShardAuthError()
                if (response.status === 429 || response.status >= 500) {
                    await this._backoff(attempt)
                    continue
                }
                if (!response.ok) {
                    const text = await response.text()
                    throw new ShardAPIError(`API error: ${text}`, response.status, text)
                }

                return (await response.json()) as ChatCompletionResponse
            } catch (e) {
                if (e instanceof ShardAuthError) throw e
                if (e instanceof ShardAPIError) throw e
                if (e instanceof TypeError && (e as Error).message.includes('fetch')) {
                    throw new ShardConnectionError(
                        `Cannot connect to Shard daemon at ${this.baseUrl}`,
                    )
                }
                if (attempt === this.maxRetries - 1) {
                    throw new ShardTimeoutError(
                        `Request failed after ${this.maxRetries} attempts`,
                    )
                }
                await this._backoff(attempt)
            }
        }

        throw new ShardAPIError('Max retries exceeded')
    }

    private async *_streamingChat(
        params: ChatCompletionRequest,
    ): AsyncGenerator<StreamChunk, void, unknown> {
        const body: ChatCompletionRequest = {
            model: 'default',
            temperature: 0.7,
            sensitive: false,
            top_p: 1.0,
            ...params,
            stream: true,
        }

        const response = await this._fetch('/v1/chat/completions', {
            method: 'POST',
            body: JSON.stringify(body),
            headers: this._headers(body),
        })

        if (response.status === 401) throw new ShardAuthError()
        if (!response.ok) {
            const text = await response.text()
            throw new ShardAPIError(`API error: ${text}`, response.status, text)
        }

        if (!response.body) {
            throw new ShardAPIError('No response body for streaming request')
        }

        const reader = response.body.getReader()
        const decoder = new TextDecoder()
        let buffer = ''

        try {
            while (true) {
                const { done, value } = await reader.read()
                if (done) break

                buffer += decoder.decode(value, { stream: true })
                const lines = buffer.split('\n')
                buffer = lines.pop() ?? ''

                for (const line of lines) {
                    const trimmed = line.trim()
                    if (!trimmed || !trimmed.startsWith('data: ')) continue

                    const data = trimmed.slice(6)
                    if (data === '[DONE]') return

                    try {
                        yield JSON.parse(data) as StreamChunk
                    } catch {
                        // Skip malformed chunks
                    }
                }
            }
        } finally {
            reader.releaseLock()
        }
    }

    private _headers(request: ChatCompletionRequest): Record<string, string> {
        const headers: Record<string, string> = {
            'Content-Type': 'application/json',
        }
        if (this.apiKey) {
            headers['Authorization'] = `Bearer ${this.apiKey}`
        }
        if (request.sensitive) {
            if (!this.apiKey) {
                console.warn('⚠️ sensitive=true but no API key set — request may be rejected')
            }
            headers['X-Shard-Route'] = 'private'
        }
        return headers
    }

    private async _fetch(
        path: string,
        init: RequestInit,
    ): Promise<Response> {
        const controller = new AbortController()
        const timer = setTimeout(() => controller.abort(), this.timeout)

        try {
            return await fetch(`${this.baseUrl}${path}`, {
                ...init,
                signal: controller.signal,
            })
        } catch (e) {
            if (e instanceof DOMException && e.name === 'AbortError') {
                throw new ShardTimeoutError(`Request timed out after ${this.timeout}ms`)
            }
            throw e
        } finally {
            clearTimeout(timer)
        }
    }

    private async _backoff(attempt: number): Promise<void> {
        const delay = RETRY_BACKOFF_BASE * 2 ** attempt
        await new Promise((resolve) => setTimeout(resolve, delay))
    }
}
