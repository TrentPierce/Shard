jest.mock("next/headers", () => ({
  headers: () => ({
    get: () => null,
  }),
}))

describe("server shard backend selection", () => {
  const originalEnv = process.env

  beforeEach(() => {
    jest.resetModules()
    process.env = { ...originalEnv }
    delete process.env.SHARD_BACKEND_URLS
    delete process.env.SHARD_BACKEND_URL
    delete process.env.NEXT_PUBLIC_SHARD_BACKEND_URLS
    delete process.env.NEXT_PUBLIC_SHARD_BACKEND_URL
    delete process.env.SHARD_FALLBACK_URLS
    delete process.env.SHARD_FALLBACK_URL
  })

  afterAll(() => {
    process.env = originalEnv
  })

  it("parses and deduplicates backend URL candidates", async () => {
    process.env.SHARD_BACKEND_URLS = "http://a:9091, http://b:9091, http://a:9091"
    process.env.SHARD_BACKEND_URL = "http://c:9091"
    const { shardBackendUrls } = await import("@/lib/server/shard-backend")

    expect(shardBackendUrls()).toEqual([
      "http://a:9091/",
      "http://b:9091/",
      "http://c:9091/",
      "https://api.shardnetwork.live/",
    ])
  })

  it("builds route URLs for all backend candidates", async () => {
    process.env.SHARD_BACKEND_URLS = "http://a:9091, http://b:9091"
    const { shardBackendUrls } = await import("@/lib/server/shard-backend")

    expect(shardBackendUrls("/health")).toEqual([
      "http://a:9091/health",
      "http://b:9091/health",
      "https://api.shardnetwork.live/health",
    ])
  })

  it("includes fallback URL when SHARD_FALLBACK_URL is set", async () => {
    process.env.SHARD_FALLBACK_URL = "http://10.0.0.5:9091"
    const { shardBackendUrls } = await import("@/lib/server/shard-backend")

    expect(shardBackendUrls("/health")).toEqual([
      "https://api.shardnetwork.live/health",
      "http://10.0.0.5:9091/health",
    ])
  })

  it("returns only default backend when no env vars are set", async () => {
    const { shardBackendUrls } = await import("@/lib/server/shard-backend")

    expect(shardBackendUrls()).toEqual([
      "https://api.shardnetwork.live/",
    ])
  })
})
