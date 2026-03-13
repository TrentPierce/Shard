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
}))

describe("execution provenance route", () => {
  it("proxies using the execution id path segment", async () => {
    const { GET } = await import(
      "@/app/api/v1/executions/[executionId]/provenance/route"
    )
    const { proxyShardJsonGet } = await import("@/lib/server/shard-json-proxy")

    const request = {
      headers: new Headers(),
      nextUrl: new URL("https://shardnetwork.live/api/v1/executions/exec-22/provenance"),
    } as any

    await GET(request, { params: { executionId: "exec-22" } })

    expect(proxyShardJsonGet).toHaveBeenCalledWith(
      request,
      "/v1/executions/exec-22/provenance",
    )
  })
})

export {}
