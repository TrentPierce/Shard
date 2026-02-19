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

  it("renders core system sections", () => {
    render(<NetworkStatus {...defaultProps} />)
    expect(screen.getByText(/Neural Core/i)).toBeInTheDocument()
    expect(screen.getByText(/Active Swarm/i)).toBeInTheDocument()
    expect(screen.getByText(/Intel Layer/i)).toBeInTheDocument()
  })

  it("shows current node mode label", () => {
    render(<NetworkStatus {...defaultProps} />)
    expect(screen.getByText(/^Scout$/i)).toBeInTheDocument()
  })

  it("renders download progress when rust status is downloading", () => {
    render(<NetworkStatus {...defaultProps} rustStatus="downloading" />)
    expect(screen.getByText(/downloading/i)).toBeInTheDocument()
  })

  it("renders webllm progress details", () => {
    render(
      <NetworkStatus
        {...defaultProps}
        webLLMProgress={{
          progress: 0.5,
          text: "Loading model...",
          timeElapsed: 5000,
        }}
      />,
    )
    expect(screen.getByText(/Scout Initializing/i)).toBeInTheDocument()
    expect(screen.getByText(/50%/i)).toBeInTheDocument()
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
