;(global as any).Request = (global as any).Request ?? (class Request {})
;(global as any).Response = (global as any).Response ?? (class Response {})
;(global as any).Headers = (global as any).Headers ?? (class Headers {})

describe("chat completions route CORS resolution", () => {
  it("allows same-origin browser requests", async () => {
    const { resolveCorsOrigin } = await import("@/app/api/v1/chat/completions/route")
    const request = {
      headers: new Headers({
        origin: "https://shardnetwork.live",
      }),
      nextUrl: new URL("https://shardnetwork.live/api/v1/chat/completions"),
    }

    expect(resolveCorsOrigin(request as never)).toBe("https://shardnetwork.live")
  })

  it("rejects foreign origins not on the allowlist", async () => {
    const { resolveCorsOrigin } = await import("@/app/api/v1/chat/completions/route")
    const request = {
      headers: new Headers({
        origin: "https://evil.example",
      }),
      nextUrl: new URL("https://shardnetwork.live/api/v1/chat/completions"),
    }

    expect(resolveCorsOrigin(request as never)).toBeNull()
  })
})
