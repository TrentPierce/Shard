import type { ChatMessage, NetworkInferenceMode } from "./api"

export type ChatRouteMode = "auto" | "browser" | "network" | "experimental-wan"

export type ChatRouteDecision =
    | {
        kind: "local_answer"
        reason: string
        complexityScore: number
        networkMode: null
        shouldCompact: boolean
    }
    | {
        kind: "network_route" | "network_route_with_compaction"
        reason: string
        complexityScore: number
        networkMode: NetworkInferenceMode
        shouldCompact: boolean
    }

type RouteInput = {
    history: ChatMessage[]
    prompt: string
    mode: ChatRouteMode
    browserRuntimeAvailable: boolean
}

const SIMPLE_PROMPT_RE =
    /\b(summarize|summary|translate|rewrite|rephrase|paraphrase|shorten|grammar|proofread|draft a|title ideas|headline ideas|bullet points)\b/i

const COMPLEX_PROMPT_RE =
    /\b(debug|refactor|implement|architecture|tradeoffs|step[- ]by[- ]step|analyze|analysis|compare|benchmark|optimize|prove|reasoning|distributed|system design|plan)\b/i

const CODE_RE = /```|function\s+\w+|class\s+\w+|console\.|stack trace|error[:\s]/i

function totalChars(messages: ChatMessage[]): number {
    return messages.reduce((sum, message) => sum + message.content.length, 0)
}

function scoreComplexity(history: ChatMessage[], prompt: string): number {
    let score = 0
    const promptChars = prompt.length
    const conversationChars = totalChars(history)
    if (promptChars > 280) score += 0.2
    if (promptChars > 700) score += 0.2
    if (conversationChars > 3500) score += 0.2
    if (history.length > 8) score += 0.15
    if (history.length > 14) score += 0.15
    if (COMPLEX_PROMPT_RE.test(prompt)) score += 0.35
    if (CODE_RE.test(prompt)) score += 0.4
    if (SIMPLE_PROMPT_RE.test(prompt)) score -= 0.2
    return Math.max(0, Math.min(1, score))
}

function shouldCompact(history: ChatMessage[]): boolean {
    const chars = totalChars(history)
    return history.length > 10 || chars > 4500
}

export function decideChatRoute(input: RouteInput): ChatRouteDecision {
    const { history, prompt, mode, browserRuntimeAvailable } = input
    const complexityScore = scoreComplexity(history, prompt)
    const compact = shouldCompact(history)

    if (mode === "browser") {
        if (browserRuntimeAvailable) {
            return {
                kind: "local_answer",
                reason: "browser_only_mode",
                complexityScore,
                networkMode: null,
                shouldCompact: compact,
            }
        }
        return {
            kind: compact ? "network_route_with_compaction" : "network_route",
            reason: "browser_only_fallback_no_runtime",
            complexityScore,
            networkMode: "standard",
            shouldCompact: compact,
        }
    }

    if (mode === "network") {
        return {
            kind: compact ? "network_route_with_compaction" : "network_route",
            reason: "network_only_mode",
            complexityScore,
            networkMode: "standard",
            shouldCompact: compact,
        }
    }

    if (mode === "experimental-wan") {
        return {
            kind: compact ? "network_route_with_compaction" : "network_route",
            reason: "experimental_wan_mode",
            complexityScore,
            networkMode: "experimental_wan",
            shouldCompact: compact,
        }
    }

    if (browserRuntimeAvailable && complexityScore <= 0.35 && !compact) {
        return {
            kind: "local_answer",
            reason: "auto_local_simple_prompt",
            complexityScore,
            networkMode: null,
            shouldCompact: false,
        }
    }

    return {
        kind: compact ? "network_route_with_compaction" : "network_route",
        reason: compact ? "auto_network_compacted_context" : "auto_network_complex_prompt",
        complexityScore,
        networkMode: "local_speculative",
        shouldCompact: compact,
    }
}
