import type { ChatRouteDecision, ChatRouteMode } from "./browser-router"

export const ROUTE_DECISION_EVENT = "shard:route-decision"

export type RouteDecisionDetail = {
    mode: ChatRouteMode
    decision: ChatRouteDecision
    browserRuntimeAvailable: boolean
    promptChars: number
    historyMessages: number
    historyChars: number
    fallback: boolean
    compactedMessages?: number
    compactedChars?: number
    originalChars?: number
    semanticBackend?: string
    semanticMessagesKept?: number
}

export type RouteAnalyticsSnapshot = {
    routeDecisions: number
    localRouteDecisions: number
    networkRouteDecisions: number
    compactedRouteDecisions: number
    browserFallbackRoutes: number
    avgRouteComplexityX100: number
    lastRouteKind: ChatRouteDecision["kind"] | "none"
    lastRouteReason: string
}

export const DEFAULT_ROUTE_ANALYTICS: RouteAnalyticsSnapshot = {
    routeDecisions: 0,
    localRouteDecisions: 0,
    networkRouteDecisions: 0,
    compactedRouteDecisions: 0,
    browserFallbackRoutes: 0,
    avgRouteComplexityX100: 0,
    lastRouteKind: "none",
    lastRouteReason: "unknown",
}

export function emitRouteDecision(detail: RouteDecisionDetail) {
    if (typeof window === "undefined") return
    window.dispatchEvent(new CustomEvent<RouteDecisionDetail>(ROUTE_DECISION_EVENT, { detail }))
}

export function reduceRouteAnalytics(
    current: RouteAnalyticsSnapshot,
    detail: RouteDecisionDetail,
): RouteAnalyticsSnapshot {
    const routeDecisions = current.routeDecisions + 1
    const complexityX100 = Math.round(detail.decision.complexityScore * 100)
    const avgRouteComplexityX100 = Math.round(
        ((current.avgRouteComplexityX100 * current.routeDecisions) + complexityX100) / routeDecisions,
    )
    const isLocal = detail.decision.kind === "local_answer"
    const isCompacted = detail.decision.kind === "network_route_with_compaction"

    return {
        routeDecisions,
        localRouteDecisions: current.localRouteDecisions + (isLocal ? 1 : 0),
        networkRouteDecisions: current.networkRouteDecisions + (isLocal ? 0 : 1),
        compactedRouteDecisions: current.compactedRouteDecisions + (isCompacted ? 1 : 0),
        browserFallbackRoutes: current.browserFallbackRoutes + (detail.fallback ? 1 : 0),
        avgRouteComplexityX100,
        lastRouteKind: detail.decision.kind,
        lastRouteReason: detail.decision.reason,
    }
}
