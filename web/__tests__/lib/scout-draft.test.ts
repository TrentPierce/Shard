import "@testing-library/jest-dom"

describe("scout-draft guardrails", () => {
  beforeEach(() => {
    jest.resetModules()
    ;(global as any).fetch = jest.fn()
    window.localStorage.clear()
    window.sessionStorage.clear()
  })

  it("retries transient submission failures and succeeds", async () => {
    const { submitDraft } = await import("@/lib/scout-draft")
    const fetchMock = global.fetch as jest.Mock

    fetchMock
      .mockResolvedValueOnce({
        ok: false,
        status: 503,
        json: async () => ({ detail: "temporary unavailable" }),
      })
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => ({ ok: true, detail: "accepted" }),
      })

    const response = await submitDraft("work-retry-1", "draft text", {
      maxRetries: 2,
      retryBackoffMs: 1,
      maxQueueDepth: 16,
    })

    expect(response.ok).toBe(true)
    expect(response.retried).toBe(1)
    expect(fetchMock).toHaveBeenCalledTimes(2)
  })

  it("rejects duplicate work ids while already queued", async () => {
    const { submitDraft } = await import("@/lib/scout-draft")
    const fetchMock = global.fetch as jest.Mock

    let releaseFirst: (() => void) | null = null
    fetchMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          releaseFirst = () =>
            resolve({
              ok: true,
              status: 200,
              json: async () => ({ ok: true }),
            })
        })
    )

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

    releaseFirst?.()
    const firstResult = await first
    expect(firstResult.ok).toBe(true)
  })

  it("marks polling result as transient error after retry budget", async () => {
    const { pollForWork } = await import("@/lib/scout-draft")
    const fetchMock = global.fetch as jest.Mock

    fetchMock.mockRejectedValue(new Error("network down"))

    const result = await pollForWork("scout-1", {
      pollRetries: 1,
      pollRetryBackoffMs: 1,
      pollTimeoutMs: 20,
    })

    expect(result.work).toBeNull()
    expect(result.transient_error).toBe(true)
    expect(result.detail).toMatch(/network down/i)
    expect(fetchMock).toHaveBeenCalledTimes(2)
  })
})
