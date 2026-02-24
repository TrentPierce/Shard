"use client"

import { FormEvent, useMemo, useRef, useState } from "react"
import { sendMessage, type ChatMessage } from "@/lib/api"

export default function ChatPage() {
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState("")
  const [streaming, setStreaming] = useState(false)
  const endRef = useRef<HTMLDivElement>(null)

  const statusText = useMemo(() => (streaming ? "Routing through distributed mesh" : "Connected to Shard Network"), [streaming])

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    const content = input.trim()
    if (!content || streaming) return

    const userMessage: ChatMessage = { role: "user", content, timestamp: Date.now() }
    setMessages((prev) => [...prev, userMessage, { role: "assistant", content: "", timestamp: Date.now() }])
    setInput("")
    setStreaming(true)

    try {
      await sendMessage(
        [...messages, userMessage],
        (token) => {
          setMessages((prev) => {
            const next = [...prev]
            const last = next[next.length - 1]
            if (last?.role === "assistant") {
              next[next.length - 1] = { ...last, content: `${last.content}${token}` }
            }
            return next
          })
          endRef.current?.scrollIntoView({ behavior: "smooth" })
        },
        () => setStreaming(false),
      )
    } catch {
      setMessages((prev) => {
        const next = [...prev]
        const last = next[next.length - 1]
        if (last?.role === "assistant") {
          next[next.length - 1] = {
            ...last,
            content: "Unable to reach the Shard backend. Verify your node or API endpoint status.",
          }
        }
        return next
      })
      setStreaming(false)
    }
  }

  return (
    <main id="main-content" className="h-[calc(100dvh-4rem)] py-4 sm:py-6">
      <section className="relative flex h-full flex-col overflow-hidden rounded-2xl border border-ring bg-base-900 shadow-panel">
        <div className="absolute right-4 top-4 flex items-center gap-2 rounded-full border border-ring bg-base-800/90 px-3 py-1 text-xs text-ink-100">
          <span className="h-2.5 w-2.5 rounded-full bg-accent-400 animate-pulseSoft" />
          {statusText}
        </div>

        <div className="flex-1 space-y-4 overflow-y-auto px-4 pb-4 pt-14 sm:px-6">
          {messages.length === 0 ? (
            <div className="mx-auto mt-12 max-w-md rounded-xl border border-ring bg-base-800 p-4 text-center sm:mt-16">
              <p className="text-sm text-ink-100">Ask anything and stream answers from the distributed inference mesh.</p>
            </div>
          ) : null}

          {messages.map((message, index) => (
            <article
              key={`${message.timestamp}-${index}`}
              className={`max-w-[85%] rounded-xl px-4 py-3 text-sm ${
                message.role === "user"
                  ? "ml-auto bg-accent-500 text-base-950"
                  : "border border-ring bg-base-800 text-ink-100"
              }`}
            >
              {message.content || "..."}
            </article>
          ))}
          <div ref={endRef} />
        </div>

        <form onSubmit={submit} className="border-t border-ring bg-base-900 p-3 sm:p-4">
          <div className="flex items-end gap-2">
            <textarea
              value={input}
              onChange={(event) => setInput(event.target.value)}
              placeholder="Type your prompt"
              rows={1}
              className="min-h-11 max-h-36 flex-1 resize-none rounded-xl border border-ring bg-base-800 px-3 py-2 text-sm text-ink-50 outline-none focus:border-accent-500"
            />
            <button
              type="submit"
              disabled={!input.trim() || streaming}
              className="inline-flex min-h-11 min-w-11 items-center justify-center rounded-xl bg-accent-500 px-4 py-2 text-sm font-medium text-base-950 transition hover:bg-accent-400 disabled:cursor-not-allowed disabled:opacity-60"
            >
              Send
            </button>
          </div>
        </form>
      </section>
    </main>
  )
}
