import {
  getChatProxySliSnapshot,
  isChatProxySliBreached,
  recordChatProxyResult,
  resetChatProxySliForTests,
} from "@/lib/server/proxy-chat-sli"

describe("proxy chat SLI", () => {
  beforeEach(() => {
    resetChatProxySliForTests()
  })

  it("does not breach under low request volume", () => {
    const now = Date.now()
    for (let i = 0; i < 5; i += 1) {
      recordChatProxyResult({
        outcome: "http_5xx",
        attempts: 1,
        fallback_used: false,
        latency_ms: 120,
        at_ms: now + i,
      })
    }
    const snapshot = getChatProxySliSnapshot(now + 10)
    expect(snapshot.window_5m.request_count).toBe(5)
    expect(isChatProxySliBreached(snapshot)).toBe(false)
  })

  it("breaches when 5xx rate exceeds threshold over sufficient samples", () => {
    const now = Date.now()
    for (let i = 0; i < 25; i += 1) {
      recordChatProxyResult({
        outcome: i < 3 ? "http_5xx" : "success",
        attempts: 2,
        fallback_used: i % 2 === 0,
        latency_ms: 200 + i,
        at_ms: now + i,
      })
    }
    const snapshot = getChatProxySliSnapshot(now + 100)
    expect(snapshot.window_5m.request_count).toBe(25)
    expect(snapshot.window_5m.http_5xx_rate).toBeGreaterThan(0.05)
    expect(isChatProxySliBreached(snapshot)).toBe(true)
  })
})
