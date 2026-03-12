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
    browserRuntimePreferred?: boolean
}

const SIMPLE_PROMPT_RE =
    /\b(summarize|summary|translate|rewrite|rephrase|paraphrase|shorten|grammar|proofread|draft a|title ideas|headline ideas|bullet points)\b/i
const EASY_LOCAL_PROMPT_RE =
    /\b(what is|what does|how does|explain|briefly explain|quick answer|quick explanation|define|tell me about)\b/i

const COMPLEX_PROMPT_RE =
    /\b(debug|refactor|implement|architecture|tradeoffs|step[- ]by[- ]step|analyze|analysis|compare|benchmark|optimize|prove|reasoning|distributed|system design|plan|deploy|production|integrate|integration|latency|throughput)\b/i

const CODE_RE = /```|function\s+\w+|class\s+\w+|console\.|stack trace|error[:\s]/i
const SYSTEM_HEAVY_RE = /\b(architecture|tradeoffs|distributed|scheduler|latency|throughput|production|integration)\b/i
const SHARD_PRODUCT_RE = /\b(shard|verifier|mesh forwarding|browser router|local-first|offload|speculative)\b/i
const SHARD_PRODUCT_SYSTEM_RE = /\b(shard|verifier|mesh|router|local-first|offload|speculative)\b.*\b(route|routing|network|latency|scheduler|forward|forwarding|architecture|prompts)\b|\b(route|routing|network|latency|scheduler|forward|forwarding|architecture|prompts)\b.*\b(shard|verifier|mesh|router|local-first|offload|speculative)\b/i
const LANGUAGE_RE = /\b(typescript|javascript|python|rust|sql|go)\b/i
const MULTISTEP_RE = /\b(first|second|third|then|finally|walk me through|how would you|design a|build a)\b/i
const SHORT_LOCAL_PROMPT_RE = /\b(explain in one paragraph|one sentence|tl;dr|quick summary|briefly)\b/i
const FOLLOW_UP_LOCAL_RE = /\b(what about|why|how so|what does that mean|can you clarify|say more|continue)\b/i

function totalChars(messages: ChatMessage[]): number {
    return messages.reduce((sum, message) => sum + message.content.length, 0)
}

function scoreComplexity(history: ChatMessage[], prompt: string): number {
    let score = 0
    const promptChars = prompt.length
    const conversationChars = totalChars(history)
    const lineCount = prompt.split(/\r?\n/).filter(Boolean).length
    if (promptChars > 280) score += 0.2
    if (promptChars > 700) score += 0.2
    if (conversationChars > 3500) score += 0.2
    if (history.length > 8) score += 0.15
    if (history.length > 14) score += 0.15
    if (lineCount > 8) score += 0.15
    if (COMPLEX_PROMPT_RE.test(prompt)) score += 0.35
    if (SYSTEM_HEAVY_RE.test(prompt)) score += 0.2
    if (SHARD_PRODUCT_RE.test(prompt)) score += 0.34
    if (LANGUAGE_RE.test(prompt)) score += 0.15
    if (MULTISTEP_RE.test(prompt)) score += 0.2
    if (CODE_RE.test(prompt)) score += 0.4
    if (SIMPLE_PROMPT_RE.test(prompt)) score -= 0.2
    if (EASY_LOCAL_PROMPT_RE.test(prompt) && promptChars < 240) score -= 0.12
    if (FOLLOW_UP_LOCAL_RE.test(prompt) && promptChars < 140 && history.length <= 8) score -= 0.08
    if (SHORT_LOCAL_PROMPT_RE.test(prompt) && promptChars < 220) score -= 0.1
    return Math.max(0, Math.min(1, score))
}

function shouldCompact(history: ChatMessage[]): boolean {
    const chars = totalChars(history)
    return history.length > 10 || chars > 4500
}

export function decideChatRoute(input: RouteInput): ChatRouteDecision {
    const { history, prompt, mode, browserRuntimeAvailable } = input
    const browserRuntimePreferred = input.browserRuntimePreferred ?? browserRuntimeAvailable
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

    if (browserRuntimeAvailable && SHARD_PRODUCT_SYSTEM_RE.test(prompt) && prompt.length >= 32) {
        return {
            kind: compact ? "network_route_with_compaction" : "network_route",
            reason: compact
                ? "auto_network_shard_context_compacted"
                : "auto_network_product_specific_prompt",
            complexityScore,
            networkMode: "standard",
            shouldCompact: compact,
        }
    }

    if (browserRuntimeAvailable && browserRuntimePreferred && complexityScore <= 0.38 && !compact) {
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
        reason: compact
            ? "auto_network_compacted_context"
            : browserRuntimeAvailable && !browserRuntimePreferred
                ? "auto_network_runtime_not_preferred"
            : complexityScore > 0.55
                ? "auto_network_heavy_prompt"
                : "auto_network_complex_prompt",
        complexityScore,
        networkMode: "standard",
        shouldCompact: compact,
    }
}
