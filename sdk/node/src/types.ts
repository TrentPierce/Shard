/**
 * TypeScript types for the Shard SDK.
 * Follows the OpenAI Chat Completions API format for drop-in compatibility.
 */

export interface ChatMessage {
    role: 'system' | 'user' | 'assistant'
    content: string
}

export interface ChatCompletionRequest {
    model?: string
    messages: ChatMessage[]
    stream?: boolean
    temperature?: number
    max_tokens?: number
    top_p?: number
    sensitive?: boolean
}

export interface StreamDelta {
    role?: string
    content?: string
}

export interface StreamChoice {
    index: number
    delta: StreamDelta
    finish_reason: string | null
}

export interface StreamChunk {
    id: string
    object: 'chat.completion.chunk'
    created: number
    model: string
    choices: StreamChoice[]
}

export interface Usage {
    prompt_tokens: number
    completion_tokens: number
    total_tokens: number
}

export interface ChatCompletionChoice {
    index: number
    message: ChatMessage
    finish_reason: string | null
}

export interface ChatCompletionResponse {
    id: string
    object: 'chat.completion'
    created: number
    model: string
    choices: ChatCompletionChoice[]
    usage: Usage
}

export interface ShardClientOptions {
    /** API key for authentication. */
    apiKey?: string
    /** Base URL of the Shard daemon API. Default: http://localhost:8080 */
    baseUrl?: string
    /** Request timeout in ms. Default: 30000. */
    timeout?: number
    /** Max retries on 429/5xx. Default: 3. */
    maxRetries?: number
}
