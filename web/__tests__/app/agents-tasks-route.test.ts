const globalAny = global as any

globalAny.Request = globalAny.Request ?? class Request {}
globalAny.Response = globalAny.Response ?? class Response {
  static json(payload: unknown) {
    return payload
  }
}
globalAny.Headers = globalAny.Headers ?? class Headers {}

jest.mock("@/lib/server/shard-json-proxy", () => ({
  proxyShardJsonPost: jest.fn(() => ({ ok: true })),
  proxyOptions: jest.fn(() => ({ ok: true })),
}))

describe("agents tasks route", () => {
  it("proxies to the daemon agents endpoint", async () => {
    const { POST } = await import("@/app/api/v1/agents/tasks/route")
    const { proxyShardJsonPost } = await import("@/lib/server/shard-json-proxy")

    const request = {
      headers: new Headers(),
      nextUrl: new URL("https://shardnetwork.live/api/v1/agents/tasks"),
      text: async () => '{"workflow_kind":"research_brief"}',
    } as any

    await POST(request)

    expect(proxyShardJsonPost).toHaveBeenCalledWith(request, "/v1/agents/tasks")
  })

  it("responds to preflight checks", async () => {
    const { OPTIONS } = await import("@/app/api/v1/agents/tasks/route")
    const { proxyOptions } = await import("@/lib/server/shard-json-proxy")

    const request = {
      headers: new Headers({ origin: "https://shardnetwork.live" }),
      nextUrl: new URL("https://shardnetwork.live/api/v1/agents/tasks"),
    } as any

    await OPTIONS(request)

    expect(proxyOptions).toHaveBeenCalledWith(request, "POST, OPTIONS")
  })
})

export {}
