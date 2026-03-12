"use client"

import { FormEvent, KeyboardEvent, useEffect, useMemo, useRef, useState } from "react"
import {
  emitChatFailure,
  sendMessage as sendNetworkMessage,
} from "@/lib/api"
import {
  sendBrowserChatMessage,
  canUseBrowserChatRuntime,
  shouldPreferBrowserChatRuntime,
} from "@/lib/browser-chat"
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

type RouteTrace = {
  id: string
  promptPreview: string
  decisionKind: ChatRouteDecision["kind"]
  decisionReason: string
  complexityScore: number
  browserRuntimeAvailable: boolean
  compacted: boolean
  semanticBackend?: string
  semanticMessagesKept?: number
  summaryChars?: number
  sentMessageCount?: number
  transport?: string
  inferenceMode?: string
  backend?: string
  backendAttempts?: number
  servedBy?: string
  meshForwarded?: boolean
  meshDecision?: string
  meshDetail?: string
  meshForwardTarget?: string
  meshTargetTier?: string
  meshForwardedBy?: string
  latencyMs?: number
  status: "pending" | "success" | "failure"
  error?: string
}

function previewPrompt(content: string): string {
  const normalized = content.replace(/\s+/g, " ").trim()
  if (normalized.length <= 88) return normalized
  return `${normalized.slice(0, 85)}...`
}

function updateTrace(
  setRouteTraces: React.Dispatch<React.SetStateAction<RouteTrace[]>>,
  traceId: string,
  patch: Partial<RouteTrace>,
) {
  setRouteTraces((prev) =>
    prev.map((trace) => (trace.id === traceId ? { ...trace, ...patch } : trace)),
  )
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
  const [routeTraces, setRouteTraces] = useState<RouteTrace[]>([])
  const endRef = useRef<HTMLDivElement>(null)
  const formRef = useRef<HTMLFormElement>(null)

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" })
  }, [messages])

  const statusText = useMemo(() => {
    if (streaming) {
      return describeActiveRoute(lastDecision)
    }
    return describeIdleMode(routeMode)
  }, [lastDecision, routeMode, streaming])

  const handleComposerKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || event.shiftKey || streaming) {
      return
    }
    event.preventDefault()
    formRef.current?.requestSubmit()
  }

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

    const [browserRuntimeAvailable, browserRuntimePreferred] = await Promise.all([
      canUseBrowserChatRuntime(),
      shouldPreferBrowserChatRuntime(),
    ])
    const decision = decideChatRoute({
      history: convo.rawMessages,
      prompt: content,
      mode: routeMode,
      browserRuntimeAvailable,
      browserRuntimePreferred,
    })
    if (decision.kind === "local_answer") {
      emitRouteDecision({
        mode: routeMode,
        decision,
        browserRuntimeAvailable,
        promptChars: content.length,
        historyMessages: convo.rawMessages.length,
        historyChars: convo.rawMessages.reduce((sum, message) => sum + message.content.length, 0),
        fallback: false,
      })
    }
    setLastDecision(decision)
    const traceId = `${userMessage.timestamp}`
    setRouteTraces((prev) => [
      {
        id: traceId,
        promptPreview: previewPrompt(content),
        decisionKind: decision.kind,
        decisionReason: decision.reason,
        complexityScore: decision.complexityScore,
        browserRuntimeAvailable,
        compacted: false,
        status: "pending",
      },
      ...prev.slice(0, 7),
    ])
    beginAssistantMessage()
    let networkAttempted = false

    try {
      if (decision.kind === "local_answer") {
        try {
          const execution = await sendBrowserChatMessage(
            convo.rawMessages,
            appendAssistantToken,
            () => undefined,
          )
          updateTrace(setRouteTraces, traceId, {
            transport: execution.transport,
            inferenceMode: execution.inferenceMode,
            latencyMs: execution.latencyMs,
            status: "success",
          })
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
            compactedMessages: fallbackUsesCompaction
              ? networkConvo.compactedMessages.length
              : networkConvo.rawMessages.length,
            compactedChars: networkConvo.compaction.compactedChars,
            originalChars: networkConvo.compaction.originalChars,
            semanticBackend: networkConvo.semantic?.backend,
            semanticMessagesKept: networkConvo.compaction.semanticMessagesKept,
          })
          setLastDecision(fallbackDecision)
          updateTrace(setRouteTraces, traceId, {
            decisionKind: fallbackDecision.kind,
            decisionReason: fallbackDecision.reason,
            compacted: fallbackUsesCompaction,
            semanticBackend: networkConvo.semantic?.backend,
            semanticMessagesKept: networkConvo.compaction.semanticMessagesKept,
            summaryChars: networkConvo.compaction.summaryChars,
            sentMessageCount: fallbackUsesCompaction
              ? networkConvo.compactedMessages.length
              : networkConvo.rawMessages.length,
          })
          replaceAssistantMessage("")
          networkAttempted = true
          const execution = await sendNetworkMessage(
            fallbackUsesCompaction ? networkConvo.compactedMessages : networkConvo.rawMessages,
            appendAssistantToken,
            () => undefined,
            fallbackDecision.networkMode,
          )
          updateTrace(setRouteTraces, traceId, {
            transport: execution.transport,
            inferenceMode: execution.inferenceMode,
            backend: execution.backend,
            backendAttempts: execution.backendAttempts,
            servedBy: execution.servedBy,
            meshForwarded: execution.meshForwarded,
            meshDecision: execution.meshDecision,
            meshDetail: execution.meshDetail,
            meshForwardTarget: execution.meshForwardTarget,
            meshTargetTier: execution.meshTargetTier,
            meshForwardedBy: execution.meshForwardedBy,
            latencyMs: execution.latencyMs,
            status: "success",
          })
        }
      } else {
        const networkConvo = await getNetworkSnapshot()
        emitRouteDecision({
          mode: routeMode,
          decision,
          browserRuntimeAvailable,
          promptChars: content.length,
          historyMessages: convo.rawMessages.length,
          historyChars: convo.rawMessages.reduce((sum, message) => sum + message.content.length, 0),
          fallback: false,
          compactedMessages:
            decision.kind === "network_route_with_compaction"
              ? networkConvo.compactedMessages.length
              : networkConvo.rawMessages.length,
          compactedChars: networkConvo.compaction.compactedChars,
          originalChars: networkConvo.compaction.originalChars,
          semanticBackend: networkConvo.semantic?.backend,
          semanticMessagesKept: networkConvo.compaction.semanticMessagesKept,
        })
        updateTrace(setRouteTraces, traceId, {
          compacted: decision.kind === "network_route_with_compaction",
          semanticBackend: networkConvo.semantic?.backend,
          semanticMessagesKept: networkConvo.compaction.semanticMessagesKept,
          summaryChars: networkConvo.compaction.summaryChars,
          sentMessageCount:
            decision.kind === "network_route_with_compaction"
              ? networkConvo.compactedMessages.length
              : networkConvo.rawMessages.length,
        })
        networkAttempted = true
        const execution = await sendNetworkMessage(
          decision.kind === "network_route_with_compaction"
            ? networkConvo.compactedMessages
            : networkConvo.rawMessages,
          appendAssistantToken,
          () => undefined,
          decision.networkMode,
        )
        updateTrace(setRouteTraces, traceId, {
          transport: execution.transport,
          inferenceMode: execution.inferenceMode,
          backend: execution.backend,
          backendAttempts: execution.backendAttempts,
          servedBy: execution.servedBy,
          meshForwarded: execution.meshForwarded,
          meshDecision: execution.meshDecision,
          meshDetail: execution.meshDetail,
          meshForwardTarget: execution.meshForwardTarget,
          meshTargetTier: execution.meshTargetTier,
          meshForwardedBy: execution.meshForwardedBy,
          latencyMs: execution.latencyMs,
          status: "success",
        })
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
      updateTrace(setRouteTraces, traceId, {
        status: "failure",
        latencyMs: Math.round(performance.now() - startedAt),
        error: String((error as Error)?.message ?? error ?? "unknown error"),
      })
      replaceAssistantMessage(
        "Unable to complete the request. If you are using Auto mode, verify the local browser runtime or daemon endpoint. For WAN scout testing, use Experimental WAN only when the verifier is prepared for it.",
      )
    } finally {
      setStreaming(false)
    }
  }

  return (
    <main
      id="main-content"
      className="box-border h-[calc(100dvh-4rem)] supports-[height:100svh]:h-[calc(100svh-4rem)] py-4 sm:py-6"
    >
      <section className="relative flex h-full min-h-0 flex-col overflow-hidden rounded-2xl border border-ring bg-base-900 shadow-panel">
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

        <div className="grid min-h-0 flex-1 gap-4 overflow-hidden px-4 pb-4 pt-20 sm:px-6 lg:grid-cols-[minmax(0,1fr)_320px]">
          <div className="min-h-0 space-y-4 overflow-y-auto pr-1">
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

          <aside className="min-h-0 overflow-y-auto rounded-xl border border-ring bg-base-800/70 p-3">
            <div className="mb-3 flex items-center justify-between">
              <h2 className="text-sm font-semibold text-ink-50">Route Trace</h2>
              <span className="text-[11px] uppercase tracking-[0.18em] text-ink-400">
                Last {routeTraces.length}
              </span>
            </div>
            <div className="space-y-3">
              {routeTraces.length === 0 ? (
                <div className="rounded-lg border border-dashed border-ring px-3 py-4 text-xs text-ink-400">
                  Send a prompt to see whether it stayed local, compacted context, hit the verifier directly, or was mesh-forwarded.
                </div>
              ) : (
                routeTraces.map((trace) => (
                  <div key={trace.id} className="rounded-lg border border-ring bg-base-900/80 px-3 py-3 text-xs text-ink-200">
                    <div className="mb-2 flex items-start justify-between gap-3">
                      <div className="font-medium text-ink-50">{trace.promptPreview}</div>
                      <span
                        className={`rounded-full px-2 py-0.5 text-[10px] uppercase tracking-[0.18em] ${
                          trace.status === "success"
                            ? "bg-emerald-500/15 text-emerald-300"
                            : trace.status === "failure"
                              ? "bg-rose-500/15 text-rose-300"
                              : "bg-amber-500/15 text-amber-300"
                        }`}
                      >
                        {trace.status}
                      </span>
                    </div>
                    <div className="space-y-1 text-ink-300">
                      <div>Decision: <span className="text-ink-100">{trace.decisionKind}</span> via <span className="text-ink-100">{trace.decisionReason}</span></div>
                      <div>Complexity: <span className="text-ink-100">{Math.round(trace.complexityScore * 100)}%</span></div>
                      <div>Browser runtime: <span className="text-ink-100">{trace.browserRuntimeAvailable ? "available" : "unavailable"}</span></div>
                      <div>Compaction: <span className="text-ink-100">{trace.compacted ? "yes" : "no"}</span>{trace.sentMessageCount ? `, sent ${trace.sentMessageCount} messages` : ""}</div>
                      {trace.semanticBackend ? (
                        <div>Semantic compaction: <span className="text-ink-100">{trace.semanticBackend}</span>{typeof trace.semanticMessagesKept === "number" ? `, kept ${trace.semanticMessagesKept} older turns` : ""}</div>
                      ) : null}
                      {trace.transport ? (
                        <div>Transport: <span className="text-ink-100">{trace.transport}</span>{trace.inferenceMode ? ` (${trace.inferenceMode})` : ""}</div>
                      ) : null}
                      {trace.backend ? (
                        <div>Backend: <span className="text-ink-100">{trace.backend}</span>{trace.backendAttempts ? `, attempts ${trace.backendAttempts}` : ""}</div>
                      ) : null}
                      {trace.servedBy ? (
                        <div>Served by: <span className="text-ink-100">{trace.servedBy}</span></div>
                      ) : null}
                      {typeof trace.meshForwarded === "boolean" ? (
                        <div>Mesh forwarded: <span className="text-ink-100">{trace.meshForwarded ? "yes" : "no"}</span></div>
                      ) : null}
                      {trace.meshDecision ? (
                        <div>Mesh decision: <span className="text-ink-100">{trace.meshDecision}</span></div>
                      ) : null}
                      {trace.meshForwardTarget ? (
                        <div>Mesh target: <span className="text-ink-100">{trace.meshForwardTarget}</span>{trace.meshTargetTier ? ` (${trace.meshTargetTier})` : ""}</div>
                      ) : null}
                      {trace.meshDetail ? (
                        <div className="break-words text-ink-400">Detail: {trace.meshDetail}</div>
                      ) : null}
                      {typeof trace.latencyMs === "number" ? (
                        <div>Latency: <span className="text-ink-100">{trace.latencyMs} ms</span></div>
                      ) : null}
                      {trace.error ? (
                        <div className="break-words text-rose-300">Error: {trace.error}</div>
                      ) : null}
                    </div>
                  </div>
                ))
              )}
            </div>
          </aside>
        </div>

        <form ref={formRef} onSubmit={submit} className="border-t border-ring bg-base-900 p-3 sm:p-4">
          <div className="flex items-end gap-2">
            <textarea
              value={input}
              onChange={(event) => setInput(event.target.value)}
              onKeyDown={handleComposerKeyDown}
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
