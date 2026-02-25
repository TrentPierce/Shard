import "@testing-library/jest-dom"

describe("scout-draft guardrails", () => {
  beforeEach(() => {
    jest.resetModules()
    ;(global as any).fetch = jest.fn()
    ;(global as any).Worker = class {
      onmessage: ((event: MessageEvent) => void) | null = null
      onerror: ((event: ErrorEvent) => void) | null = null
      constructor(_url?: any, _opts?: any) {}
      postMessage(_data: any) {
        const payload = { type: "solved", nonce: 0, hashHex: "00".repeat(32), elapsedMs: 1 }
        this.onmessage?.({ data: payload } as MessageEvent)
      }
      terminate() {}
    }
    window.localStorage.clear()
    window.sessionStorage.clear()
  })

  it("retries transient submission failures and succeeds", async () => {
    const { submitDraft } = await import("@/lib/scout-draft")
    const fetchMock = global.fetch as jest.Mock

    let draftAttempt = 0
    fetchMock.mockImplementation(async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url.includes("/v1/pow/challenge")) {
        return {
          ok: true,
          status: 200,
          json: async () => ({
            ok: true,
            challenge: {
              challenge_bytes_hex: "00".repeat(32),
              difficulty: 12,
            },
          }),
        }
      }
      if (url.includes("/v1/pow/verify")) {
        return {
          ok: true,
          status: 200,
          json: async () => ({ ok: true }),
        }
      }
      if (url.includes("/v1/scout/draft")) {
        draftAttempt += 1
        if (draftAttempt === 1) {
          return {
            ok: false,
            status: 503,
            json: async () => ({ detail: "temporary unavailable" }),
          }
        }
        return {
          ok: true,
          status: 200,
          json: async () => ({ ok: true, detail: "accepted" }),
        }
      }
      throw new Error(`Unhandled fetch URL: ${url}`)
    })

    const response = await submitDraft("work-retry-1", "draft text", {
      maxRetries: 2,
      retryBackoffMs: 1,
      maxQueueDepth: 16,
    })

    expect(response.ok).toBe(true)
    expect(response.retried).toBe(1)
    // Includes challenge+verify+2 draft calls plus best-effort client telemetry calls.
    expect(fetchMock.mock.calls.length).toBeGreaterThanOrEqual(4)
  })

  it("rejects duplicate work ids while already queued", async () => {
    const { submitDraft } = await import("@/lib/scout-draft")
    const fetchMock = global.fetch as jest.Mock

    let releaseFirst: (() => void) | null = null
    fetchMock.mockImplementation((input: RequestInfo | URL) => {
      const url = String(input)
      if (url.includes("/v1/pow/challenge")) {
        return Promise.resolve({
          ok: true,
          status: 200,
          json: async () => ({
            ok: true,
            challenge: {
              challenge_bytes_hex: "00".repeat(32),
              difficulty: 12,
            },
          }),
        })
      }
      if (url.includes("/v1/pow/verify")) {
        return Promise.resolve({
          ok: true,
          status: 200,
          json: async () => ({ ok: true }),
        })
      }
      if (url.includes("/v1/scout/draft")) {
        return new Promise((resolve) => {
          releaseFirst = () =>
            resolve({
              ok: true,
              status: 200,
              json: async () => ({ ok: true }),
            })
        })
      }
      return Promise.reject(new Error(`Unhandled fetch URL: ${url}`))
    })

    const first = submitDraft("work-dup-1", "draft one", {
      maxRetries: 0,
      maxQueueDepth: 16,
    })
    const second = await submitDraft("work-dup-1", "draft one", {
      maxRetries: 0,
      maxQueueDepth: 16,
    })

    expect(second.ok).toBe(false)
    expect(second.detail).toMatch(/duplicate work_id/i)

    for (let i = 0; i < 20 && !releaseFirst; i += 1) {
      await new Promise((resolve) => setTimeout(resolve, 10))
    }
    releaseFirst?.()
    const firstResult = await first
    expect(firstResult.ok).toBe(true)
  })

  it("marks polling result as transient error after retry budget", async () => {
    const { pollForWork } = await import("@/lib/scout-draft")
    const fetchMock = global.fetch as jest.Mock

    let pollAttempts = 0
    fetchMock.mockImplementation(async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url.includes("/v1/pow/challenge")) {
        return {
          ok: true,
          status: 200,
          json: async () => ({
            ok: true,
            challenge: {
              challenge_bytes_hex: "00".repeat(32),
              difficulty: 12,
            },
          }),
        }
      }
      if (url.includes("/v1/pow/verify")) {
        return {
          ok: true,
          status: 200,
          json: async () => ({ ok: true }),
        }
      }
      if (url.includes("/v1/scout/work")) {
        pollAttempts += 1
        throw new Error("network down")
      }
      throw new Error(`Unhandled fetch URL: ${url}`)
    })

    const result = await pollForWork("scout-1", {
      pollRetries: 1,
      pollRetryBackoffMs: 1,
      pollTimeoutMs: 20,
    })

    expect(result.work).toBeNull()
    expect(result.transient_error).toBe(true)
    expect(result.detail).toMatch(/network down/i)
    // 2 PoW calls (challenge+verify) + 2 poll attempts.
    expect(fetchMock).toHaveBeenCalledTimes(4)
    expect(pollAttempts).toBe(2)
  })
})
