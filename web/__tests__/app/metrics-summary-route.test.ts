jest.mock("next/headers", () => ({
  headers: () => ({
    get: () => null,
  }),
}));

(global as any).Request = (global as any).Request ?? class Request {};
(global as any).Response = (global as any).Response ?? class Response {};
(global as any).Headers = (global as any).Headers ?? class Headers {};

describe("metrics summary node estimation", () => {
  const originalEnv = process.env
  const originalFetch = global.fetch

  beforeEach(() => {
    jest.resetModules()
    process.env = { ...originalEnv }
    process.env.SHARD_BACKEND_URLS = "http://node-a:9091,http://node-b:9091"
    global.fetch = jest.fn(async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url === "http://node-a:9091/health") {
        return {
          ok: true,
          json: async () => ({
            status: "ok",
            ready_for_inference: true,
            model_id: "meta-llama/Llama-3.2-1B",
            rust_version: "0.6.6",
          }),
        } as unknown as Response
      }
      if (url === "http://node-b:9091/health") {
        return {
          ok: true,
          json: async () => ({
            status: "degraded",
            ready_for_inference: false,
            readiness_reason: "queue_backpressure",
            model_id: "meta-llama/Llama-3.2-1B",
            rust_version: "0.6.6",
          }),
        } as unknown as Response
      }
      if (url === "https://api.shardnetwork.live/health") {
        return {
          ok: false,
          status: 503,
          json: async () => ({}),
        } as unknown as Response
      }
      if (url === "https://shard-fly-bench-0308c.fly.dev/health") {
        return {
          ok: false,
          status: 503,
          json: async () => ({}),
        } as unknown as Response
      }
      if (url.endsWith("/v1/system/peers")) {
        return {
          ok: true,
          json: async () => ({ count: 25 }),
        } as unknown as Response
      }
      throw new Error(`Unexpected fetch ${url}`)
    }) as typeof fetch
  })

  afterAll(() => {
    process.env = originalEnv
    global.fetch = originalFetch
  })

  it("prefers backend health probes over peer-count estimation", async () => {
    const { deriveActiveNodeEstimate } = await import("@/app/api/v1/metrics/summary/route")

    const result = await deriveActiveNodeEstimate("http://node-a:9091/metrics/summary", 0)

    expect(result.activeNodes).toBe(2)
    expect(result.reachableNodes).toBe(2)
    expect(result.healthyNodes).toBe(1)
    expect(result.unhealthyNodes).toBe(1)
    expect(result.activeNodesEstimateSource).toBe("backend_health_probe")
    expect(result.nodeProbes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          backend: "http://node-a:9091",
          healthy: true,
        }),
        expect.objectContaining({
          backend: "http://node-b:9091",
          healthy: false,
          readiness_reason: "queue_backpressure",
        }),
      ]),
    )
  })
})

