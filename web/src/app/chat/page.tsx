"use client"

import { FormEvent, useEffect, useMemo, useRef, useState } from "react"
import { emitChatFailure, sendMessage as sendNetworkMessage } from "@/lib/api"
import { sendBrowserChatMessage, canUseBrowserChatRuntime } from "@/lib/browser-chat"
import {
  decideChatRoute,
  type ChatRouteDecision,
  type ChatRouteMode,
} from "@/lib/browser-router"
import { useConversationState } from "@/lib/conversation-state"
import { emitRouteDecision } from "@/lib/route-telemetry"

function describeIdleMode(mode: ChatRouteMode) {
  switch (mode) {
    case "browser":
      return "Browser-only mode"
    case "network":
      return "Network-only mode"
    case "experimental-wan":
      return "Experimental WAN mode"
    default:
      return "Auto route: browser first, network for harder prompts"
  }
}

function describeActiveRoute(decision: ChatRouteDecision | null) {
  if (!decision) {
    return "Preparing route"
  }
  if (decision.kind === "local_answer") {
    return "Answering locally in the browser"
  }
  if (decision.kind === "network_route_with_compaction") {
    return `Routing compacted context to ${decision.networkMode.replace("_", " ")}`
  }
  return `Routing to ${decision.networkMode.replace("_", " ")}`
}

export default function ChatPage() {
  const {
    messages,
    appendUserMessage,
    beginAssistantMessage,
    appendAssistantToken,
    replaceAssistantMessage,
    snapshot,
    snapshotForNetwork,
  } = useConversationState()
  const [input, setInput] = useState("")
  const [streaming, setStreaming] = useState(false)
  const [routeMode, setRouteMode] = useState<ChatRouteMode>("auto")
  const [lastDecision, setLastDecision] = useState<ChatRouteDecision | null>(null)
  const endRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" })
  }, [messages])

  const statusText = useMemo(() => {
    if (streaming) {
      return describeActiveRoute(lastDecision)
    }
    return describeIdleMode(routeMode)
  }, [lastDecision, routeMode, streaming])

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    const content = input.trim()
    if (!content || streaming) return

    const userMessage = appendUserMessage(content)
    const convo = snapshot(userMessage)
    setInput("")
    setStreaming(true)
    const startedAt = performance.now()
    let networkSnapshotPromise: Promise<typeof convo> | null = null

    const getNetworkSnapshot = () => {
      if (!networkSnapshotPromise) {
        networkSnapshotPromise = snapshotForNetwork(convo.rawMessages, content)
      }
      return networkSnapshotPromise
    }

    const browserRuntimeAvailable = await canUseBrowserChatRuntime()
    const decision = decideChatRoute({
      history: convo.rawMessages,
      prompt: content,
      mode: routeMode,
      browserRuntimeAvailable,
    })
    emitRouteDecision({
      mode: routeMode,
      decision,
      browserRuntimeAvailable,
      promptChars: content.length,
      historyMessages: convo.rawMessages.length,
      historyChars: convo.rawMessages.reduce((sum, message) => sum + message.content.length, 0),
      fallback: false,
    })
    setLastDecision(decision)
    beginAssistantMessage()
    let networkAttempted = false

    try {
      if (decision.kind === "local_answer") {
        try {
          await sendBrowserChatMessage(
            convo.rawMessages,
            appendAssistantToken,
            () => undefined,
          )
        } catch (error) {
          if (routeMode !== "auto") {
            throw error
          }
          const networkConvo = await getNetworkSnapshot()
          const fallbackUsesCompaction = networkConvo.compaction.wasCompacted
          const fallbackDecision: ChatRouteDecision = {
            kind: fallbackUsesCompaction
              ? "network_route_with_compaction"
              : "network_route",
            reason: "auto_local_failed_network_fallback",
            complexityScore: decision.complexityScore,
            networkMode: "standard",
            shouldCompact: fallbackUsesCompaction,
          }
          emitRouteDecision({
            mode: routeMode,
            decision: fallbackDecision,
            browserRuntimeAvailable,
            promptChars: content.length,
            historyMessages: convo.rawMessages.length,
            historyChars: convo.rawMessages.reduce((sum, message) => sum + message.content.length, 0),
            fallback: true,
          })
          setLastDecision(fallbackDecision)
          replaceAssistantMessage("")
          networkAttempted = true
          await sendNetworkMessage(
            fallbackUsesCompaction ? networkConvo.compactedMessages : networkConvo.rawMessages,
            appendAssistantToken,
            () => undefined,
            fallbackDecision.networkMode,
          )
        }
      } else {
        const networkConvo = await getNetworkSnapshot()
        networkAttempted = true
        await sendNetworkMessage(
          decision.kind === "network_route_with_compaction"
            ? networkConvo.compactedMessages
            : networkConvo.rawMessages,
          appendAssistantToken,
          () => undefined,
          decision.networkMode,
        )
      }
    } catch (error) {
      if (decision.kind === "local_answer" && !networkAttempted) {
        emitChatFailure({
          latencyMs: Math.round(performance.now() - startedAt),
          inferenceMode: "browser_local",
          transport: "browser_local",
          error: String((error as Error)?.message ?? error ?? "unknown error"),
        })
      }
      replaceAssistantMessage(
        "Unable to complete the request. If you are using Auto mode, verify the local browser runtime or daemon endpoint. For WAN scout testing, use Experimental WAN only when the verifier is prepared for it.",
      )
    } finally {
      setStreaming(false)
    }
  }

  return (
    <main id="main-content" className="h-[calc(100dvh-4rem)] py-4 sm:py-6">
      <section className="relative flex h-full flex-col overflow-hidden rounded-2xl border border-ring bg-base-900 shadow-panel">
        <div className="absolute right-4 top-4 flex flex-col items-end gap-2">
          <div className="flex items-center gap-2 rounded-full border border-ring bg-base-800/90 px-3 py-1 text-xs text-ink-100">
            <span className="h-2.5 w-2.5 rounded-full bg-accent-400 animate-pulseSoft" />
            {statusText}
          </div>
          <select
            value={routeMode}
            onChange={(event) => setRouteMode(event.target.value as ChatRouteMode)}
            className="rounded-full border border-ring bg-base-800/90 px-3 py-1 text-xs text-ink-100 outline-none"
          >
            <option value="auto">Auto</option>
            <option value="browser">Browser Only</option>
            <option value="network">Network Only</option>
            <option value="experimental-wan">Experimental WAN</option>
          </select>
        </div>

        <div className="flex-1 space-y-4 overflow-y-auto px-4 pb-4 pt-20 sm:px-6">
          {messages.length === 0 ? (
            <div className="mx-auto mt-12 max-w-xl rounded-xl border border-ring bg-base-800 p-4 text-center sm:mt-16">
              <p className="text-sm text-ink-100">
                Ask anything. Simple prompts can finish locally in the browser, while heavier prompts route to the desktop verifier path.
              </p>
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
