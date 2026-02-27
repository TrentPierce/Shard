export type ChatProxyOutcome =
  | "success"
  | "http_5xx"
  | "timeout"
  | "other_error"

type ChatProxySample = {
  at_ms: number
  outcome: ChatProxyOutcome
  attempts: number
  fallback_used: boolean
  latency_ms: number
}

type ChatProxyCounters = {
  requests_total: number
  success_total: number
  http_5xx_total: number
  timeout_total: number
  other_error_total: number
}

export type ChatProxyWindowSnapshot = {
  window_ms: number
  request_count: number
  success_count: number
  http_5xx_count: number
  timeout_count: number
  other_error_count: number
  http_5xx_rate: number
  timeout_rate: number
  error_rate: number
}

export type ChatProxySliSnapshot = {
  counters: ChatProxyCounters
  window_5m: ChatProxyWindowSnapshot
  generated_at_ms: number
}

const MAX_SAMPLE_HISTORY = 10_000
const DEFAULT_HISTORY_MS = 15 * 60 * 1000
const DEFAULT_WINDOW_MS = 5 * 60 * 1000
const DEFAULT_MIN_WINDOW_REQUESTS = 20
const DEFAULT_MAX_5XX_RATE = 0.05

let samples: ChatProxySample[] = []
let counters: ChatProxyCounters = {
  requests_total: 0,
  success_total: 0,
  http_5xx_total: 0,
  timeout_total: 0,
  other_error_total: 0,
}

function parsePositiveInt(raw: string | undefined, fallback: number): number {
  const parsed = Number.parseInt(raw ?? "", 10)
  if (!Number.isFinite(parsed) || parsed <= 0) return fallback
  return parsed
}

function parsePositiveFloat(raw: string | undefined, fallback: number): number {
  const parsed = Number.parseFloat(raw ?? "")
  if (!Number.isFinite(parsed) || parsed <= 0) return fallback
  return parsed
}

function historyRetentionMs(): number {
  return parsePositiveInt(process.env.SHARD_PROXY_SLI_HISTORY_MS, DEFAULT_HISTORY_MS)
}

function windowMs(): number {
  return parsePositiveInt(process.env.SHARD_PROXY_SLI_WINDOW_MS, DEFAULT_WINDOW_MS)
}

function minWindowRequests(): number {
  return parsePositiveInt(
    process.env.SHARD_PROXY_SLI_MIN_WINDOW_REQUESTS,
    DEFAULT_MIN_WINDOW_REQUESTS,
  )
}

function max5xxRate(): number {
  return parsePositiveFloat(
    process.env.SHARD_PROXY_SLI_MAX_5XX_RATE,
    DEFAULT_MAX_5XX_RATE,
  )
}

function pruneSamples(nowMs: number): void {
  const cutoff = nowMs - historyRetentionMs()
  if (samples.length > MAX_SAMPLE_HISTORY) {
    samples = samples.slice(-MAX_SAMPLE_HISTORY)
  }
  if (samples.length === 0) return
  let firstFresh = 0
  while (firstFresh < samples.length && samples[firstFresh].at_ms < cutoff) {
    firstFresh += 1
  }
  if (firstFresh > 0) {
    samples = samples.slice(firstFresh)
  }
}

function buildWindowSnapshot(
  nowMs: number,
  requestedWindowMs: number,
): ChatProxyWindowSnapshot {
  const cutoff = nowMs - requestedWindowMs
  let requestCount = 0
  let successCount = 0
  let http5xxCount = 0
  let timeoutCount = 0
  let otherErrorCount = 0

  for (const sample of samples) {
    if (sample.at_ms < cutoff) continue
    requestCount += 1
    switch (sample.outcome) {
      case "success":
        successCount += 1
        break
      case "http_5xx":
        http5xxCount += 1
        break
      case "timeout":
        timeoutCount += 1
        break
      case "other_error":
        otherErrorCount += 1
        break
      default:
        break
    }
  }

  const denom = requestCount > 0 ? requestCount : 1
  const errorCount = http5xxCount + timeoutCount + otherErrorCount

  return {
    window_ms: requestedWindowMs,
    request_count: requestCount,
    success_count: successCount,
    http_5xx_count: http5xxCount,
    timeout_count: timeoutCount,
    other_error_count: otherErrorCount,
    http_5xx_rate: http5xxCount / denom,
    timeout_rate: timeoutCount / denom,
    error_rate: errorCount / denom,
  }
}

export function recordChatProxyResult(input: {
  outcome: ChatProxyOutcome
  attempts: number
  fallback_used: boolean
  latency_ms: number
  at_ms?: number
}): void {
  const nowMs = input.at_ms ?? Date.now()
  counters.requests_total += 1
  switch (input.outcome) {
    case "success":
      counters.success_total += 1
      break
    case "http_5xx":
      counters.http_5xx_total += 1
      break
    case "timeout":
      counters.timeout_total += 1
      break
    case "other_error":
      counters.other_error_total += 1
      break
    default:
      break
  }

  samples.push({
    at_ms: nowMs,
    outcome: input.outcome,
    attempts: Math.max(1, input.attempts),
    fallback_used: input.fallback_used,
    latency_ms: Math.max(0, Math.round(input.latency_ms)),
  })
  pruneSamples(nowMs)
}

export function getChatProxySliSnapshot(nowMs = Date.now()): ChatProxySliSnapshot {
  pruneSamples(nowMs)
  return {
    counters: { ...counters },
    window_5m: buildWindowSnapshot(nowMs, windowMs()),
    generated_at_ms: nowMs,
  }
}

export function isChatProxySliBreached(snapshot: ChatProxySliSnapshot): boolean {
  const window = snapshot.window_5m
  if (window.request_count < minWindowRequests()) return false
  return window.http_5xx_rate > max5xxRate()
}

export function resetChatProxySliForTests(): void {
  samples = []
  counters = {
    requests_total: 0,
    success_total: 0,
    http_5xx_total: 0,
    timeout_total: 0,
    other_error_total: 0,
  }
}
