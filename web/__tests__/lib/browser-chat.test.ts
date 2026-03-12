import {
  canUseBrowserChatRuntime,
  sendBrowserChatMessage,
  shouldPreferBrowserChatRuntime,
} from "@/lib/browser-chat"

jest.mock("@/lib/webllm", () => ({
  checkWebGPUSupport: jest.fn(),
  generateBrowserChatCompletion: jest.fn(),
  isModelCached: jest.fn(),
}))

const { checkWebGPUSupport, generateBrowserChatCompletion, isModelCached } = jest.requireMock("@/lib/webllm") as {
  checkWebGPUSupport: jest.Mock
  generateBrowserChatCompletion: jest.Mock
  isModelCached: jest.Mock
}

describe("browser chat", () => {
  const originalNavigator = global.navigator

  beforeEach(() => {
    jest.clearAllMocks()
    Object.defineProperty(global, "navigator", {
      value: { userAgent: "Mozilla/5.0 Chrome/122.0" },
      configurable: true,
    })
  })

  afterAll(() => {
    Object.defineProperty(global, "navigator", {
      value: originalNavigator,
      configurable: true,
    })
  })

  it("reports runtime support when WebGPU is available", async () => {
    checkWebGPUSupport.mockResolvedValue({ supported: true })

    await expect(canUseBrowserChatRuntime()).resolves.toBe(true)
  })

  it("does not prefer uncached browser runtimes in auto mode", async () => {
    checkWebGPUSupport.mockResolvedValue({ supported: true })
    isModelCached.mockResolvedValue(false)

    await expect(shouldPreferBrowserChatRuntime()).resolves.toBe(false)
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
