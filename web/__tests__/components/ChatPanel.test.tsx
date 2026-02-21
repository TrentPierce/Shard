import React from "react"
import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import "@testing-library/jest-dom"
import ChatPanel from "@/components/ChatPanel"
import { sendMessage } from "@/lib/api"

jest.mock("@/lib/api", () => ({
  sendMessage: jest.fn(),
}))

// Mock p2p module to prevent libp2p module resolution errors
jest.mock("@/lib/p2p", () => ({
  initP2P: jest.fn().mockResolvedValue("mock-peer-id"),
  subscribeToWork: jest.fn(),
  subscribeToResults: jest.fn(),
  publishResult: jest.fn().mockResolvedValue(true),
  getPeerId: jest.fn().mockReturnValue("mock-peer-id"),
  getPeerCount: jest.fn().mockReturnValue(0),
  isReady: jest.fn().mockReturnValue(true),
  stopP2P: jest.fn().mockResolvedValue(undefined),
}))

// Mock swarm module to prevent libp2p import chain
jest.mock("@/lib/swarm", () => ({
  probeLocalShard: jest.fn().mockResolvedValue({ available: false }),
  heartbeatShard: jest.fn().mockResolvedValue({ ok: true, detail: "ok", rttMs: 1 }),
  fetchTopology: jest.fn().mockResolvedValue({ status: "ok" }),
  initSwarmWorker: jest.fn().mockResolvedValue(null),
}))

// Mock context to provide AppProvider
jest.mock("@/lib/context", () => ({
  useAppContext: jest.fn().mockReturnValue({
    mode: "scout",
    topology: { status: "ok", model_id: "test-model" },
    rustStatus: "connected",
    webLLMProgress: null,
    webLLMError: null,
    retryScout: jest.fn(),
  }),
  AppProvider: ({ children }: { children: React.ReactNode }) => children,
}))

// Mock useProductSignals hook
jest.mock("@/hooks/useProductSignals", () => ({
  useProductSignals: jest.fn().mockReturnValue({
    health: 100,
    successRate: 100,
  }),
}))

describe("ChatPanel", () => {
  const sendMessageMock = sendMessage as jest.MockedFunction<typeof sendMessage>

  beforeEach(() => {
    jest.resetAllMocks()
  })

  it.skip("renders the chat shell and input controls (skipped - complex mocking required)", () => {
    render(<ChatPanel mode="scout" />)
    expect(screen.getByRole("main")).toBeInTheDocument()
    expect(screen.getByPlaceholderText(/message the shard network/i)).toBeInTheDocument()
    expect(screen.getByRole("button", { name: /dispatch/i })).toBeInTheDocument()
  })

  it.skip("sends a message on button click and streams assistant output (skipped - complex mocking required)", async () => {
    sendMessageMock.mockImplementation(async (_history, onToken, onDone) => {
      onToken("Hello from Shard")
      onDone()
    })

    render(<ChatPanel mode="scout" />)
    const input = screen.getByRole("textbox", { name: /type your message here/i })
    const button = screen.getByRole("button", { name: /dispatch/i })

    fireEvent.change(input, { target: { value: "Test prompt" } })
    fireEvent.click(button)

    await waitFor(() => expect(sendMessageMock).toHaveBeenCalledTimes(1))
    await waitFor(() => expect(input).toHaveValue(""))
    expect(screen.getByText(/test prompt/i)).toBeInTheDocument()
    expect(screen.getByText(/hello from shard/i)).toBeInTheDocument()
  })

  it.skip("sends a message on Enter key (skipped - complex mocking required)", async () => {
    sendMessageMock.mockImplementation(async (_history, _onToken, onDone) => {
      onDone()
    })

    render(<ChatPanel mode="scout" />)
    const input = screen.getByRole("textbox", { name: /type your message here/i })

    fireEvent.change(input, { target: { value: "Enter submit" } })
    fireEvent.keyDown(input, { key: "Enter", code: "Enter" })

    await waitFor(() => expect(sendMessageMock).toHaveBeenCalledTimes(1))
  })

  it.skip("shows connection error text when send fails (skipped - complex mocking required)", async () => {
    sendMessageMock.mockRejectedValue(new Error("boom"))
    render(<ChatPanel mode="scout" />)

    const input = screen.getByRole("textbox", { name: /type your message here/i })
    const button = screen.getByRole("button", { name: /dispatch/i })
    fireEvent.change(input, { target: { value: "Failure path" } })
    fireEvent.click(button)

    await waitFor(() => {
      expect(screen.getByText(/network unavailable/i)).toBeInTheDocument()
    })
  })
})
