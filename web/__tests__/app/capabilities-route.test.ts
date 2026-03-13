const globalAny = global as any

globalAny.Request = globalAny.Request ?? class Request {}
globalAny.Response = globalAny.Response ?? class Response {
  static json(payload: unknown) {
    return payload
  }
}
globalAny.Headers = globalAny.Headers ?? class Headers {}

jest.mock("@/lib/server/shard-json-proxy", () => ({
  proxyShardJsonGet: jest.fn(() => ({ ok: true })),
  proxyOptions: jest.fn(() => ({ ok: true })),
}))

describe("capabilities route", () => {
  it("proxies to the daemon capabilities endpoint", async () => {
    const { GET } = await import("@/app/api/v1/capabilities/route")
    const { proxyShardJsonGet } = await import("@/lib/server/shard-json-proxy")

    const request = {
      headers: new Headers(),
      nextUrl: new URL("https://shardnetwork.live/api/v1/capabilities"),
    } as any

    await GET(request)

    expect(proxyShardJsonGet).toHaveBeenCalledWith(request, "/v1/capabilities")
  })

  it("responds to preflight checks", async () => {
    const { OPTIONS } = await import("@/app/api/v1/capabilities/route")
    const { proxyOptions } = await import("@/lib/server/shard-json-proxy")

    const request = {
      headers: new Headers({ origin: "https://shardnetwork.live" }),
      nextUrl: new URL("https://shardnetwork.live/api/v1/capabilities"),
    } as any

    await OPTIONS(request)

    expect(proxyOptions).toHaveBeenCalledWith(request, "GET, OPTIONS")
  })
})

export {}
