import { canUseBrowserChatRuntime, sendBrowserChatMessage } from "@/lib/browser-chat"

jest.mock("@/lib/webllm", () => ({
  checkWebGPUSupport: jest.fn(),
  generateBrowserChatCompletion: jest.fn(),
}))

const { checkWebGPUSupport, generateBrowserChatCompletion } = jest.requireMock("@/lib/webllm") as {
  checkWebGPUSupport: jest.Mock
  generateBrowserChatCompletion: jest.Mock
}

describe("browser chat", () => {
  beforeEach(() => {
    jest.clearAllMocks()
  })

  it("reports runtime support when WebGPU is available", async () => {
    checkWebGPUSupport.mockResolvedValue({ supported: true })

    await expect(canUseBrowserChatRuntime()).resolves.toBe(true)
  })

  it("streams a local browser answer and emits completion", async () => {
    const onToken = jest.fn()
    const onDone = jest.fn()
    const successListener = jest.fn()
    window.addEventListener("shard:chat-success", successListener as EventListener)
    generateBrowserChatCompletion.mockImplementation(async (_history, options) => {
      options.onToken("Hello")
      options.onToken(" world")
      return { success: true, content: "Hello world" }
    })

    await sendBrowserChatMessage(
      [{ role: "user", content: "Hello?", timestamp: 1 }],
      onToken,
      onDone,
    )

    expect(onToken).toHaveBeenCalledWith("Hello")
    expect(onToken).toHaveBeenCalledWith(" world")
    expect(onDone).toHaveBeenCalledTimes(1)
    expect(successListener).toHaveBeenCalledTimes(1)
    window.removeEventListener("shard:chat-success", successListener as EventListener)
  })

  it("raises when local generation fails", async () => {
    generateBrowserChatCompletion.mockResolvedValue({
      success: false,
      error: "runtime unavailable",
    })

    await expect(
      sendBrowserChatMessage(
        [{ role: "user", content: "Hello?", timestamp: 1 }],
        () => undefined,
        () => undefined,
      ),
    ).rejects.toThrow("runtime unavailable")
  })
})
