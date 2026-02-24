import { sendMessage, sendMessageSync, type ChatMessage } from "@/lib/api"

const canUseLocalDaemonFallbackMock = jest.fn(() => false)

jest.mock("@/lib/runtime", () => ({
  canUseLocalDaemonFallback: () => canUseLocalDaemonFallbackMock(),
  localDaemonUrl: (path: string) => `http://127.0.0.1:9091${path.startsWith("/") ? path : `/${path}`}`,
}))

describe("api transport fallbacks", () => {
  const history: ChatMessage[] = [{ role: "user", content: "hello", timestamp: 1 }]

  beforeEach(() => {
    jest.restoreAllMocks()
    global.fetch = jest.fn()
  })

  it("falls back to non-streaming chat when ReadableStream reader is unavailable", async () => {
    canUseLocalDaemonFallbackMock.mockReturnValue(false)
    ;(global.fetch as jest.Mock)
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        statusText: "OK",
        body: null,
      })
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        statusText: "OK",
        json: async () => ({
          choices: [{ message: { content: "fallback<|eot_id|> reply" } }],
        }),
      })

    const onToken = jest.fn()
    const onDone = jest.fn()
    await sendMessage(history, onToken, onDone, "standard")

    expect(onToken).toHaveBeenCalledWith("fallback reply")
    expect(onDone).toHaveBeenCalledTimes(1)
    expect(global.fetch).toHaveBeenCalledTimes(2)

    const syncBody = JSON.parse((global.fetch as jest.Mock).mock.calls[1][1].body)
    expect(syncBody.stream).toBe(false)
  })

  it("does not attempt localhost fallback on hosted contexts when primary request fails", async () => {
    canUseLocalDaemonFallbackMock.mockReturnValue(false)
    ;(global.fetch as jest.Mock).mockRejectedValue(new Error("network fail"))

    await expect(sendMessageSync(history)).rejects.toThrow("Primary API")
    expect(global.fetch).toHaveBeenCalledTimes(1)
  })

  it("uses localhost fallback when explicitly enabled", async () => {
    canUseLocalDaemonFallbackMock.mockReturnValue(true)
    ;(global.fetch as jest.Mock)
      .mockRejectedValueOnce(new Error("primary down"))
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        statusText: "OK",
        json: async () => ({
          choices: [{ message: { content: "from local<|eot_id|> daemon" } }],
        }),
      })

    const content = await sendMessageSync(history)
    expect(content).toBe("from local daemon")
    expect(global.fetch).toHaveBeenCalledTimes(2)
    expect((global.fetch as jest.Mock).mock.calls[1][0]).toContain("127.0.0.1:9091")
  })
})
