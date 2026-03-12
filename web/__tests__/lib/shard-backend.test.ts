jest.mock("next/headers", () => ({
  headers: () => ({
    get: () => null,
  }),
}))

describe("server shard backend selection", () => {
  const originalEnv = process.env
  const originalFetch = global.fetch

  beforeEach(() => {
    jest.resetModules()
    process.env = { ...originalEnv }
    process.env.NODE_ENV = "test"
    delete process.env.SHARD_BACKEND_URLS
    delete process.env.SHARD_BACKEND_URL
    delete process.env.NEXT_PUBLIC_SHARD_BACKEND_URLS
    delete process.env.NEXT_PUBLIC_SHARD_BACKEND_URL
    delete process.env.SHARD_FALLBACK_URLS
    delete process.env.SHARD_FALLBACK_URL
    global.fetch = originalFetch
  })

  afterAll(() => {
    process.env = originalEnv
    global.fetch = originalFetch
  })

  it("parses and deduplicates backend URL candidates", async () => {
    process.env.SHARD_BACKEND_URLS = "http://a:9091, http://b:9091, http://a:9091"
    process.env.SHARD_BACKEND_URL = "http://c:9091"
    const { shardBackendUrls } = await import("@/lib/server/shard-backend")

    expect(shardBackendUrls()).toEqual([
      "http://a:9091/",
      "http://b:9091/",
      "http://c:9091/",
      "http://127.0.0.1:9091/",
      "http://127.0.0.1:9191/",
      "http://localhost:9091/",
      "http://localhost:9191/",
      "https://shard-fly-bench-0308c.fly.dev/",
      "https://api.shardnetwork.live/",
    ])
  })

  it("builds route URLs for all backend candidates", async () => {
    process.env.SHARD_BACKEND_URLS = "http://a:9091, http://b:9091"
    const { shardBackendUrls } = await import("@/lib/server/shard-backend")

    expect(shardBackendUrls("/health")).toEqual([
      "http://a:9091/health",
      "http://b:9091/health",
      "http://127.0.0.1:9091/health",
      "http://127.0.0.1:9191/health",
      "http://localhost:9091/health",
      "http://localhost:9191/health",
      "https://shard-fly-bench-0308c.fly.dev/health",
      "https://api.shardnetwork.live/health",
    ])
  })

  it("includes fallback URL when SHARD_FALLBACK_URL is set", async () => {
    process.env.SHARD_FALLBACK_URL = "http://10.0.0.5:9091"
    const { shardBackendUrls } = await import("@/lib/server/shard-backend")

    expect(shardBackendUrls("/health")).toEqual([
      "http://127.0.0.1:9091/health",
      "http://127.0.0.1:9191/health",
      "http://localhost:9091/health",
      "http://localhost:9191/health",
      "https://shard-fly-bench-0308c.fly.dev/health",
      "https://api.shardnetwork.live/health",
      "http://10.0.0.5:9091/health",
    ])
  })

  it("returns only default backend when no env vars are set", async () => {
    const { shardBackendUrls } = await import("@/lib/server/shard-backend")

    expect(shardBackendUrls()).toEqual([
      "http://127.0.0.1:9091/",
      "http://127.0.0.1:9191/",
      "http://localhost:9091/",
      "http://localhost:9191/",
      "https://shard-fly-bench-0308c.fly.dev/",
      "https://api.shardnetwork.live/",
    ])
  })

  it("prioritizes explicit preferred candidates during failover", async () => {
    process.env.SHARD_BACKEND_URLS = "http://a:9091, http://b:9091"
    global.fetch = jest.fn(async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url.startsWith("http://b:9091/health")) {
        return {
          ok: true,
          status: 200,
          url,
          json: async () => ({ ok: true }),
        } as unknown as Response
      }
      return {
        ok: false,
        status: 503,
        url,
        json: async () => ({ ok: false }),
      } as unknown as Response
    }) as typeof fetch

    const { fetchWithBackendFailover } = await import("@/lib/server/shard-backend")
    const result = await fetchWithBackendFailover("/health", {
      method: "GET",
      maxAttempts: 2,
      preferredCandidates: ["http://b:9091/health"],
      loadAware: false,
      failoverOnStatuses: [503],
    })

    expect(String((global.fetch as jest.Mock).mock.calls[0][0])).toBe("http://b:9091/health")
    expect(result.backend).toContain("http://b:9091/health")
  })
})
