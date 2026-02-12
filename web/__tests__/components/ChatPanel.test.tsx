import React from "react"
import { render, screen, fireEvent, waitFor } from "@testing-library/react"
import "@testing-library/jest-dom"
import ChatPanel from "@/components/ChatPanel"

describe("ChatPanel", () => {
  const defaultProps = {
    mode: "scout" as const,
  }

  beforeEach(() => {
    // Mock fetch API
    global.fetch = jest.fn(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve({}),
      } as Response)
    )
  })

  afterEach(() => {
    jest.clearAllMocks()
  })

  describe("Rendering", () => {
    it("renders chat input and button", () => {
      render(<ChatPanel {...defaultProps} />)
      expect(screen.getByPlaceholderText(/Type your message/i)).toBeInTheDocument()
      expect(screen.getByRole("button", { name: /send/i })).toBeInTheDocument()
    })

    it("renders chat history container", () => {
      render(<ChatPanel {...defaultProps} />)
      expect(screen.getByTestId("chat-history")).toBeInTheDocument()
    })

    it("displays mode indicator", () => {
      render(<ChatPanel {...defaultProps} />)
      expect(screen.getByText(/Scout Mode/i)).toBeInTheDocument()
    })
  })

  describe("Message sending", () => {
    it("sends message on button click", async () => {
      render(<ChatPanel {...defaultProps} />)

      const input = screen.getByPlaceholderText(/Type your message/i)
      const sendButton = screen.getByRole("button", { name: /send/i })

      fireEvent.change(input, { target: { value: "Hello, Shard!" } })
      fireEvent.click(sendButton)

      await waitFor(() => {
        expect(input).toHaveValue("")
      })
    })

    it("sends message on Enter key", async () => {
      render(<ChatPanel {...defaultProps} />)

      const input = screen.getByPlaceholderText(/Type your message/i)

      fireEvent.change(input, { target: { value: "Hello, Shard!" } })
      fireEvent.keyDown(input, { key: "Enter", code: "Enter" })

      await waitFor(() => {
        expect(input).toHaveValue("")
      })
    })

    it("does not send empty message", async () => {
      render(<ChatPanel {...defaultProps} />)

      const sendButton = screen.getByRole("button", { name: /send/i })

      fireEvent.click(sendButton)

      await waitFor(() => {
        expect(global.fetch).not.toHaveBeenCalled()
      })
    })
  })

  describe("Error handling", () => {
    it("displays error message when API call fails", async () => {
      const mockFetch = jest.fn(() =>
        Promise.resolve({
          ok: false,
          status: 500,
          json: () => Promise.resolve({ error: "Internal Server Error" }),
        } as Response)
      )
      global.fetch = mockFetch

      render(<ChatPanel {...defaultProps} />)

      const input = screen.getByPlaceholderText(/Type your message/i)
      const sendButton = screen.getByRole("button", { name: /send/i })

      fireEvent.change(input, { target: { value: "Test" } })
      fireEvent.click(sendButton)

      await waitFor(() => {
        expect(screen.getByText(/Failed to send message/i)).toBeInTheDocument()
      })
    })
  })
})
