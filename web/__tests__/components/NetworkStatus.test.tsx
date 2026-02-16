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
    global.fetch = jest.fn(async () =>
      ({
        ok: true,
        json: async () => ({ peers: [] }),
      }) as Response
    )
  })

  describe("Rendering in Scout mode", () => {
    it("displays scout status", () => {
      render(<NetworkStatus {...defaultProps} />)
      expect(screen.getByText(/Scout Mode Active/i)).toBeInTheDocument()
    })

    it("displays WebGPU support status", () => {
      render(<NetworkStatus {...defaultProps} />)
      expect(screen.getByText(/WebLLM/i)).toBeInTheDocument()
    })
  })

  describe("Rendering in Leech mode", () => {
    it("displays leech status with queue warning", () => {
      render(<NetworkStatus {...defaultProps} mode="leech" />)
      expect(screen.getByText(/Leech Mode Active/i)).toBeInTheDocument()
      expect(screen.getByText(/^Low$/i)).toBeInTheDocument()
    })

    it("displays upgrade prompt", () => {
      render(<NetworkStatus {...defaultProps} mode="leech" />)
      expect(screen.getByRole("button", { name: /enable scout mode/i })).toBeInTheDocument()
    })
  })

  describe("Rendering in Shard mode", () => {
    it("displays shard status", () => {
      render(<NetworkStatus {...defaultProps} mode="local-shard" />)
      expect(screen.getByText(/Daemon Status/i)).toBeInTheDocument()
      expect(screen.getByText(/Shard/i)).toBeInTheDocument()
    })

    it("displays peer count when connected", () => {
      const propsWithTopology = {
        ...defaultProps,
        mode: "local-shard",
        topology: {
          status: "ok",
          shard_peer_id: "test-peer-id",
          listen_addrs: ["/ip4/127.0.0.1/tcp/4001/ws"],
          shard_ws_multiaddr: "/ip4/127.0.0.1/tcp/4101/ws/p2p/test-peer-id",
        },
      }
      render(<NetworkStatus {...propsWithTopology} />)
      expect(screen.getByText(/WS Addr/i)).toBeInTheDocument()
      expect(screen.getByText(/✓ available/i)).toBeInTheDocument()
    })
  })

  describe("WebLLM progress", () => {
    it("displays model loading progress", () => {
      const propsWithProgress = {
        ...defaultProps,
        webLLMProgress: {
          progress: 0.5,
          text: "Loading model...",
          timeElapsed: 5000,
        },
      }
      render(<NetworkStatus {...propsWithProgress} />)
      expect(screen.getByText(/Loading model/i)).toBeInTheDocument()
      expect(screen.getByText(/50%/i)).toBeInTheDocument()
    })
  })

  describe("Error states", () => {
    it("displays WebLLM error message", () => {
      const propsWithError = {
        ...defaultProps,
        webLLMError: "WebGPU not supported on this device",
      }
      render(<NetworkStatus {...propsWithError} />)
      expect(screen.getByText(/WebGPU not supported/i)).toBeInTheDocument()
    })

    it("displays network unreachable status", () => {
      const propsWithUnreachable = {
        ...defaultProps,
        rustStatus: "unreachable",
      }
      render(<NetworkStatus {...propsWithUnreachable} />)
      expect(screen.getByText(/unreachable/i)).toBeInTheDocument()
    })
  })
})
