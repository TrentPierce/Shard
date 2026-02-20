/**
 * Shard Node.js SDK
 *
 * @example
 * ```ts
 * import { ShardClient } from '@shard-ai/sdk'
 *
 * const client = new ShardClient({ apiKey: 'sk_...' })
 *
 * // Simple usage
 * const response = await client.complete('Hello!')
 * console.log(response.choices[0].message.content)
 *
 * // OpenAI-compatible
 * const res = await client.chat.completions.create({
 *   messages: [{ role: 'user', content: 'Hello!' }],
 *   model: 'default',
 * })
 *
 * // Streaming
 * for await (const chunk of client.chat.completions.createStream({
 *   messages: [{ role: 'user', content: 'Tell me a story' }],
 * })) {
 *   process.stdout.write(chunk.choices[0]?.delta?.content ?? '')
 * }
 *
 * // Private routing for sensitive data
 * const privateRes = await client.complete('Confidential query', { sensitive: true })
 * ```
 */

export { ShardClient } from './client.js'
export type {
    ChatMessage,
    ChatCompletionRequest,
    ChatCompletionResponse,
    ChatCompletionChoice,
    StreamDelta,
    StreamChoice,
    StreamChunk,
    Usage,
    ShardClientOptions,
} from './types.js'
export {
    ShardError,
    ShardAPIError,
    ShardTimeoutError,
    ShardConnectionError,
    ShardAuthError,
} from './errors.js'
