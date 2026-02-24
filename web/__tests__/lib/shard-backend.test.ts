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
    process.env.SHARD_BACKEND_URLS = "http://a:9091, http://b:9091\nhttp://a:9091"
    process.env.SHARD_BACKEND_URL = "http://c:9091"
    const { getShardBackendBaseUrls } = await import("@/lib/server/shard-backend")

    expect(getShardBackendBaseUrls()).toEqual([
      "http://a:9091",
      "http://b:9091",
      "http://c:9091",
      "http://35.175.242.222:9091",
    ])
  })

  it("builds route URLs for all backend candidates", async () => {
    process.env.SHARD_BACKEND_URLS = "http://a:9091 http://b:9091"
    const { shardBackendUrls } = await import("@/lib/server/shard-backend")

    expect(shardBackendUrls("/health")).toEqual([
      "http://a:9091/health",
      "http://b:9091/health",
      "http://35.175.242.222:9091/health",
    ])
  })

  it("supports explicit fallback candidate list", async () => {
    process.env.SHARD_FALLBACK_URLS = "http://f1:9091;http://f2:9091"
    const { getFallbackBackendUrls } = await import("@/lib/server/shard-backend")

    expect(getFallbackBackendUrls()).toEqual([
      "http://f1:9091",
      "http://f2:9091",
      "http://35.175.242.222:9091",
    ])
  })
})
