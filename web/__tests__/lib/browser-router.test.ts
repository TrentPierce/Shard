import { decideChatRoute } from "@/lib/browser-router"
import type { ChatMessage } from "@/lib/api"

function message(content: string, role: ChatMessage["role"] = "user"): ChatMessage {
  return { role, content, timestamp: 1 }
}

describe("browser router", () => {
  it("keeps short simple prompts local when browser runtime is available", () => {
    const decision = decideChatRoute({
      history: [message("Summarize this article in one paragraph.")],
      prompt: "Summarize Shard in one paragraph.",
      mode: "auto",
      browserRuntimeAvailable: true,
    })

    expect(decision.kind).toBe("local_answer")
    expect(decision.reason).toBe("auto_local_simple_prompt")
  })

  it("routes code and architecture prompts to the network path", () => {
    const decision = decideChatRoute({
      history: [message("Help debug this TypeScript error")],
      prompt: "Refactor this distributed scheduler and explain the architecture tradeoffs.",
      mode: "auto",
      browserRuntimeAvailable: true,
    })

    expect(decision.kind).toBe("network_route")
    expect(decision.networkMode).toBe("standard")
    expect(decision.complexityScore).toBeGreaterThan(0.5)
  })

  it("compacts long conversations before routing to the network", () => {
    const history = Array.from({ length: 12 }, (_, idx) =>
      message(`Turn ${idx} ${"x".repeat(420)}`, idx % 2 === 0 ? "user" : "assistant"),
    )
    const decision = decideChatRoute({
      history,
      prompt: "Keep going.",
      mode: "auto",
      browserRuntimeAvailable: true,
    })

    expect(decision.kind).toBe("network_route_with_compaction")
    expect(decision.shouldCompact).toBe(true)
  })

  it("falls back to network in browser mode when no browser runtime is available", () => {
    const decision = decideChatRoute({
      history: [message("Translate this to Spanish")],
      prompt: "Translate this to Spanish.",
      mode: "browser",
      browserRuntimeAvailable: false,
    })

    expect(decision.kind).toBe("network_route")
    expect(decision.reason).toBe("browser_only_fallback_no_runtime")
    expect(decision.networkMode).toBe("standard")
  })

  it("preserves explicit experimental wan routing", () => {
    const decision = decideChatRoute({
      history: [message("Benchmark this route")],
      prompt: "Run the experimental path.",
      mode: "experimental-wan",
      browserRuntimeAvailable: true,
    })

    expect(decision.kind).toBe("network_route")
    expect(decision.networkMode).toBe("experimental_wan")
  })
})
