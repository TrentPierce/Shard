import React from "react"
import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import "@testing-library/jest-dom"
import ChatPanel from "@/components/ChatPanel"
import { sendMessage } from "@/lib/api"

jest.mock("@/lib/api", () => ({
  sendMessage: jest.fn(),
}))

describe("ChatPanel", () => {
  const sendMessageMock = sendMessage as jest.MockedFunction<typeof sendMessage>

  beforeEach(() => {
    jest.resetAllMocks()
  })

  it("renders the chat shell and input controls", () => {
    render(<ChatPanel mode="scout" />)
    expect(screen.getByRole("main", { name: /chat interface/i })).toBeInTheDocument()
    expect(screen.getByPlaceholderText(/ask a question/i)).toBeInTheDocument()
    expect(screen.getByRole("button", { name: /send message/i })).toBeInTheDocument()
  })

  it("sends a message on button click and streams assistant output", async () => {
    sendMessageMock.mockImplementation(async (_history, onToken, onDone) => {
      onToken("Hello from Shard")
      onDone()
    })

    render(<ChatPanel mode="scout" />)
    const input = screen.getByRole("textbox", { name: /type your message here/i })
    const button = screen.getByRole("button", { name: /send message/i })

    fireEvent.change(input, { target: { value: "Test prompt" } })
    fireEvent.click(button)

    await waitFor(() => expect(sendMessageMock).toHaveBeenCalledTimes(1))
    await waitFor(() => expect(input).toHaveValue(""))
    expect(screen.getByText(/test prompt/i)).toBeInTheDocument()
    expect(screen.getByText(/hello from shard/i)).toBeInTheDocument()
  })

  it("sends a message on Enter key", async () => {
    sendMessageMock.mockImplementation(async (_history, _onToken, onDone) => {
      onDone()
    })

    render(<ChatPanel mode="scout" />)
    const input = screen.getByRole("textbox", { name: /type your message here/i })

    fireEvent.change(input, { target: { value: "Enter submit" } })
    fireEvent.keyDown(input, { key: "Enter", code: "Enter" })

    await waitFor(() => expect(sendMessageMock).toHaveBeenCalledTimes(1))
  })

  it("shows connection error text when send fails", async () => {
    sendMessageMock.mockRejectedValue(new Error("boom"))
    render(<ChatPanel mode="scout" />)

    const input = screen.getByRole("textbox", { name: /type your message here/i })
    const button = screen.getByRole("button", { name: /send message/i })
    fireEvent.change(input, { target: { value: "Failure path" } })
    fireEvent.click(button)

    await waitFor(() => {
      expect(screen.getByText(/connection error/i)).toBeInTheDocument()
    })
  })
})
