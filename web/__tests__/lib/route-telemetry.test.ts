import {
  DEFAULT_ROUTE_ANALYTICS,
  reduceRouteAnalytics,
  type RouteDecisionDetail,
} from "@/lib/route-telemetry"

const localDecision: RouteDecisionDetail = {
  mode: "auto",
  decision: {
    kind: "local_answer",
    reason: "auto_local_simple_prompt",
    complexityScore: 0.2,
    networkMode: null,
    shouldCompact: false,
  },
  browserRuntimeAvailable: true,
  promptChars: 42,
  historyMessages: 1,
  historyChars: 42,
  fallback: false,
}

describe("route telemetry reducer", () => {
  it("tracks local route decisions", () => {
    const next = reduceRouteAnalytics(DEFAULT_ROUTE_ANALYTICS, localDecision)

    expect(next.routeDecisions).toBe(1)
    expect(next.localRouteDecisions).toBe(1)
    expect(next.networkRouteDecisions).toBe(0)
    expect(next.avgRouteComplexityX100).toBe(20)
    expect(next.lastRouteKind).toBe("local_answer")
  })

  it("tracks compacted fallback network routes", () => {
    const next = reduceRouteAnalytics(
      reduceRouteAnalytics(DEFAULT_ROUTE_ANALYTICS, localDecision),
      {
        ...localDecision,
        fallback: true,
        decision: {
          kind: "network_route_with_compaction",
          reason: "auto_local_failed_network_fallback",
          complexityScore: 0.7,
          networkMode: "local_speculative",
          shouldCompact: true,
        },
      },
    )

    expect(next.routeDecisions).toBe(2)
    expect(next.networkRouteDecisions).toBe(1)
    expect(next.compactedRouteDecisions).toBe(1)
    expect(next.browserFallbackRoutes).toBe(1)
    expect(next.avgRouteComplexityX100).toBe(45)
    expect(next.lastRouteReason).toBe("auto_local_failed_network_fallback")
  })
})
