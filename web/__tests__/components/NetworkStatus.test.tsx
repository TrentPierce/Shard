import React from "react"
import { render, screen } from "@testing-library/react"
import "@testing-library/jest-dom"
import NetworkStatus from "@/components/NetworkStatus"

describe("NetworkStatus", () => {
  const defaultProps = {
    mode: "scout" as const,
    topology: null,
    rustStatus: "unreachable" as const,
    webLLMProgress: null,
    webLLMError: null,
  }

  describe("Rendering in Scout mode", () => {
    it("displays scout status", () => {
      render(<NetworkStatus {...defaultProps} />)
      expect(screen.getByText(/Scout Node/i)).toBeInTheDocument()
    })

    it("displays WebGPU support status", () => {
      render(<NetworkStatus {...defaultProps} />)
      expect(screen.getByText(/WebGPU.*Supported/i)).toBeInTheDocument()
    })
  })

  describe("Rendering in Leech mode", () => {
    it("displays leech status with queue warning", () => {
      render(<NetworkStatus {...defaultProps} mode="leech" />)
      expect(screen.getByText(/Consumer Node/i)).toBeInTheDocument()
      expect(screen.getByText(/Low Priority/i)).toBeInTheDocument()
    })

    it("displays upgrade prompt", () => {
      render(<NetworkStatus {...defaultProps} mode="leech" />)
      expect(screen.getByText(/Enable Scout Mode/i)).toBeInTheDocument()
    })
  })

  describe("Rendering in Oracle mode", () => {
    it("displays oracle status", () => {
      render(<NetworkStatus {...defaultProps} mode="local-oracle" />)
      expect(screen.getByText(/Local Oracle/i)).toBeInTheDocument()
    })

    it("displays peer count when connected", () => {
      const propsWithTopology = {
        ...defaultProps,
        mode: "local-oracle",
        topology: {
          status: "ok",
          oracle_peer_id: "test-peer-id",
          listen_addrs: ["/ip4/127.0.0.1/tcp/4001/ws"],
        },
      }
      render(<NetworkStatus {...propsWithTopology} />)
      expect(screen.getByText(/Connected/i)).toBeInTheDocument()
    })
  })

  describe("WebLLM progress", () => {
    it("displays model loading progress", () => {
      const propsWithProgress = {
        ...defaultProps,
        webLLMProgress: {
          progress: 50,
          loaded: 100,
          total: 200,
          text: "Loading model...",
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
