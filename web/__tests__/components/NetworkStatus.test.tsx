import React from "react"
import { render, screen } from "@testing-library/react"
import "@testing-library/jest-dom"
import NetworkStatus from "@/components/NetworkStatus"

jest.mock("@/lib/swarm", () => ({
  heartbeatShard: jest.fn(async () => ({ ok: true, detail: "ok", rttMs: 1 })),
}))

describe("NetworkStatus", () => {
  const defaultProps = {
    mode: "scout" as const,
    topology: null,
    rustStatus: "unreachable" as const,
    webLLMProgress: null,
    webLLMError: null,
  }

  beforeEach(() => {
    global.fetch = jest.fn(() =>
      Promise.resolve({
        ok: true,
        json: async () => ({ peers: [] }),
      } as unknown as Response),
    )
  })

  it("renders mode label based on node mode", () => {
    render(<NetworkStatus {...defaultProps} mode="scout" />)
    expect(screen.getByText(/Scout/i)).toBeInTheDocument()
  })

  it("renders rust status", () => {
    render(<NetworkStatus {...defaultProps} rustStatus="unreachable" />)
    expect(screen.getByText(/unreachable/i)).toBeInTheDocument()
  })

  it("renders download progress when rust status is downloading", () => {
    render(<NetworkStatus {...defaultProps} rustStatus="downloading" />)
    expect(screen.getByText(/downloading/i)).toBeInTheDocument()
  })

  it("renders webllm progress details", () => {
    const { container } = render(
      <NetworkStatus
        {...defaultProps}
        webLLMProgress={{
          progress: 0.5,
          text: "Loading model...",
          timeElapsed: 5000,
        }}
      />,
    )
    expect(screen.getByText(/Loading model/i)).toBeInTheDocument()
    const progressBar = container.querySelector('div[style*="width: 50%"]')
    expect(progressBar).toBeInTheDocument()
  })

  it("renders webllm error details", () => {
    render(
      <NetworkStatus
        {...defaultProps}
        webLLMError="WebGPU not supported on this device"
      />,
    )
    expect(screen.getByText(/WebGPU not supported/i)).toBeInTheDocument()
  })
})
