import { headers } from "next/headers"

// Primary public verifier pool.
const DEFAULT_BACKENDS = [
  process.env.SHARD_PUBLIC_BACKEND_URL || "https://shard-fly-bench-0308c.fly.dev",
  // Legacy hostname retained as a fallback until the custom domain is repaired.
  "https://api.shardnetwork.live",
]
// Additional fallback: read from env so no IP is hardcoded in source
const DEFAULT_FALLBACK = process.env.SHARD_FALLBACK_URL || ""
const DEFAULT_LOCAL_BACKENDS =
  process.env.NODE_ENV === "production"
    ? []
    : [
        "http://127.0.0.1:9091",
        "http://127.0.0.1:9191",
        "http://localhost:9091",
        "http://localhost:9191",
      ]

function normalizeUrl(url: string): string {
  return url.trim().replace(/\/$/, "")
}

function parseUrlList(envValue: string | undefined): string[] {
  if (!envValue) return []
  return envValue
    .split(",")
    .map((u) => u.trim())
    .filter((u) => u.length > 0)
    .map(normalizeUrl)
}

function dedupe(urls: string[]): string[] {
  return Array.from(new Set(urls))
}

export function preferredBackendCandidatesFromHeaders(path: string = ""): string[] | undefined {
  try {
    const incoming = headers()
    const raw = incoming.get("x-shard-backend-url") || ""
    if (!raw.trim()) return undefined
    const cleanPath = path.startsWith("/") ? path : `/${path}`
    const candidates = dedupe(
      raw
        .split(",")
        .map((item) => normalizeUrl(item))
        .filter((item) => item.length > 0)
        .map((base) => `${base}${cleanPath}`),
    )
    return candidates.length > 0 ? candidates : undefined
  } catch {
    return undefined
  }
}

type BackendRuntimeState = {
  inflight: number
  ewmaLatencyMs: number
  errorEwma: number
  queueDepth: number
  p95LatencyMs: number
  cooldownUntilMs: number
  lastProbeMs: number
  lastTouchedMs: number
}

const backendRuntime = new Map<string, BackendRuntimeState>()
const BACKEND_STATE_TTL_MS = 10 * 60 * 1000
const BACKEND_PROBE_INTERVAL_MS = 2_000
const BACKEND_PROBE_TIMEOUT_MS = 700
const BACKEND_COOLDOWN_BASE_MS = 600
const BACKEND_COOLDOWN_MAX_MS = 12_000
const MAX_LOAD_HINT_PROBES_PER_CALL = 2

function applyEwma(previous: number, sample: number, alpha = 0.2): number {
  const boundedSample = Number.isFinite(sample) ? sample : previous
  return ((1 - alpha) * previous) + (alpha * boundedSample)
}

function backendBase(url: string): string {
  try {
    const parsed = new URL(url)
    return `${parsed.protocol}//${parsed.host}`
  } catch {
    return normalizeUrl(url)
  }
}

function ensureBackendState(url: string): BackendRuntimeState {
  const base = backendBase(url)
  const existing = backendRuntime.get(base)
  if (existing) {
    existing.lastTouchedMs = Date.now()
    return existing
  }
  const created: BackendRuntimeState = {
    inflight: 0,
    ewmaLatencyMs: 900,
    errorEwma: 0,
    queueDepth: 0,
    p95LatencyMs: 0,
    cooldownUntilMs: 0,
    lastProbeMs: 0,
    lastTouchedMs: Date.now(),
  }
  backendRuntime.set(base, created)
  return created
}

function pruneBackendRuntimeState(nowMs: number): void {
  for (const [base, state] of backendRuntime.entries()) {
    if (nowMs - state.lastTouchedMs > BACKEND_STATE_TTL_MS) {
      backendRuntime.delete(base)
    }
  }
}

async function fetchBackendLoadHint(base: string): Promise<{ queueDepth: number; p95LatencyMs: number } | null> {
  try {
    const response = await fetch(`${base}/metrics/summary`, {
      method: "GET",
      cache: "no-store",
      signal: AbortSignal.timeout(BACKEND_PROBE_TIMEOUT_MS),
    })
    if (!response.ok) return null
    const data = await response.json().catch(() => ({}))
    const queueDepth = Number(data?.queue_depth ?? 0)
    const p95LatencyMs = Number(data?.p95_latency_ms ?? 0)
    return {
      queueDepth: Number.isFinite(queueDepth) ? Math.max(0, queueDepth) : 0,
      p95LatencyMs: Number.isFinite(p95LatencyMs) ? Math.max(0, p95LatencyMs) : 0,
    }
  } catch {
    return null
  }
}

async function refreshLoadHints(candidates: string[]): Promise<void> {
  const nowMs = Date.now()
  pruneBackendRuntimeState(nowMs)
  const uniqueBases = dedupe(candidates.map(backendBase))
  const staleBases = uniqueBases.filter((base) => {
    const state = ensureBackendState(base)
    return nowMs - state.lastProbeMs >= BACKEND_PROBE_INTERVAL_MS
  })
  if (staleBases.length === 0) return

  const limitedBases = staleBases.slice(0, MAX_LOAD_HINT_PROBES_PER_CALL)
  await Promise.all(
    limitedBases.map(async (base) => {
      const state = ensureBackendState(base)
      state.lastProbeMs = nowMs
      const hint = await fetchBackendLoadHint(base)
      if (!hint) return
      state.queueDepth = applyEwma(state.queueDepth, hint.queueDepth, 0.35)
      state.p95LatencyMs = applyEwma(state.p95LatencyMs || hint.p95LatencyMs, hint.p95LatencyMs, 0.35)
      state.lastTouchedMs = nowMs
    }),
  )
}

function scoreBackend(url: string, nowMs: number): number {
  const state = ensureBackendState(url)
  const cooldownPenalty = state.cooldownUntilMs > nowMs
    ? 4_000 + ((state.cooldownUntilMs - nowMs) / 8)
    : 0
  return (
    (state.inflight * 90) +
    state.ewmaLatencyMs +
    (state.queueDepth * 240) +
    (state.p95LatencyMs * 0.12) +
    (state.errorEwma * 3_000) +
    cooldownPenalty
  )
}

function orderByRuntimeScore(candidates: string[]): string[] {
  const nowMs = Date.now()
  return [...candidates].sort((a, b) => scoreBackend(a, nowMs) - scoreBackend(b, nowMs))
}

function prioritizePreferredCandidates(
  candidates: string[],
  preferredCandidates: string[] | undefined,
): string[] {
  if (!preferredCandidates || preferredCandidates.length === 0) {
    return candidates
  }
  const preferred = new Set(preferredCandidates.map(normalizeUrl))
  const top: string[] = []
  const rest: string[] = []
  for (const candidate of candidates) {
    if (preferred.has(normalizeUrl(candidate))) {
      top.push(candidate)
    } else {
      rest.push(candidate)
    }
  }
  return [...top, ...rest]
}

function noteBackendAttempt(url: string): void {
  const state = ensureBackendState(url)
  state.inflight += 1
  state.lastTouchedMs = Date.now()
}

function noteBackendResult(url: string, latencyMs: number, status?: number, threw = false): void {
  const nowMs = Date.now()
  const state = ensureBackendState(url)
  state.inflight = Math.max(0, state.inflight - 1)
  state.ewmaLatencyMs = applyEwma(state.ewmaLatencyMs, Math.max(0, latencyMs), 0.22)

  let errorSample = 0
  if (threw) {
    errorSample = 1
  } else if ((status ?? 0) >= 500) {
    errorSample = 1
  } else if (status === 429) {
    errorSample = 0.6
  } else if ((status ?? 0) >= 400) {
    errorSample = 0.35
  }
  state.errorEwma = applyEwma(state.errorEwma, errorSample, 0.25)

  if (errorSample >= 1) {
    const nextCooldownMs = Math.min(
      BACKEND_COOLDOWN_MAX_MS,
      Math.max(BACKEND_COOLDOWN_BASE_MS, state.errorEwma * BACKEND_COOLDOWN_MAX_MS),
    )
    state.cooldownUntilMs = nowMs + Math.floor(nextCooldownMs)
  } else if (errorSample <= 0.05) {
    state.cooldownUntilMs = 0
  }
  state.lastTouchedMs = nowMs
}

export function shardBackendUrls(path: string = ""): string[] {
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  const multi = parseUrlList(process.env.SHARD_BACKEND_URLS)
  const single = parseUrlList(
    process.env.SHARD_BACKEND_URL || process.env.NEXT_PUBLIC_SHARD_BACKEND_URL,
  )
  const defaults = [...DEFAULT_LOCAL_BACKENDS, ...DEFAULT_BACKENDS, DEFAULT_FALLBACK]
    .map(normalizeUrl)
    .filter((u) => u.length > 0)
  return dedupe([...multi, ...single, ...defaults]).map((base) => `${base}${cleanPath}`)
}

async function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms))
}

export type FetchWithFailoverOptions = {
  method?: "GET" | "POST" | "PUT" | "DELETE"
  headers?: HeadersInit
  body?: BodyInit | null
  timeoutMs?: number
  totalTimeoutMs?: number
  maxAttempts?: number
  retryJitterMs?: number
  failoverOnStatuses?: number[]
  preferredCandidates?: string[]
  loadAware?: boolean
}

/**
 * Executes a fetch against multiple backend candidates with automatic failover.
 */
export async function fetchWithBackendFailover(
  path: string,
  options: FetchWithFailoverOptions = {},
): Promise<{ response: Response; backend: string; attempts: number }> {
  const {
    method = "GET",
    headers: requestHeaders = {},
    body = null,
    timeoutMs = 6_000,
    totalTimeoutMs = 15_000,
    maxAttempts = 3,
    retryJitterMs = 250,
    failoverOnStatuses = [500, 502, 503, 504, 521, 530],
    preferredCandidates,
    loadAware = true,
  } = options

  let candidates = shardBackendUrls(path)
  if (preferredCandidates && preferredCandidates.length > 0) {
    candidates = dedupe([
      ...preferredCandidates.map(normalizeUrl),
      ...candidates,
    ])
  }
  candidates = prioritizePreferredCandidates(candidates, preferredCandidates)
  if (loadAware && candidates.length > 1) {
    await refreshLoadHints(candidates)
    candidates = orderByRuntimeScore(candidates)
    // Preserve explicit affinity at the front if one was provided.
    candidates = prioritizePreferredCandidates(candidates, preferredCandidates)
  }
  const limitedCandidates = candidates.slice(0, maxAttempts)
  const startTime = Date.now()

  let lastError: unknown = null
  let attempts = 0

  for (let i = 0; i < limitedCandidates.length; i++) {
    let backend = limitedCandidates[i]
    attempts++

    if (Date.now() - startTime > totalTimeoutMs) {
      throw new Error(`Total failover timeout exceeded (${totalTimeoutMs}ms)`)
    }

    const attemptTimeoutMs = Math.min(timeoutMs, totalTimeoutMs - (Date.now() - startTime))
    if (attemptTimeoutMs <= 0) {
      throw new Error("Total failover timeout exceeded during attempt preparation")
    }

    // Prepare fetchInit
    const fetchInit: RequestInit = {
      method,
      headers: requestHeaders,
      body,
      signal: AbortSignal.timeout(attemptTimeoutMs),
      cache: "no-store",
    }

    try {
      console.info(`[backend] Fetching candidate ${backend} (attempt ${i + 1}/${limitedCandidates.length})`);
      const attemptStartedAt = Date.now()
      noteBackendAttempt(backend)

      // Automatic HTTPS -> HTTP fallback for raw IP addresses BEFORE the actual fetch
      // Cloudflare Edge Runtime often fails immediately on HTTPS to raw IP.
      const urlObj = new URL(backend)
      const isRawIp = /^\d{1,3}(\.\d{1,3}){3}/.test(urlObj.hostname)
      if (isRawIp && urlObj.protocol === "https:") {
        console.warn(`[backend] Raw IP detected with HTTPS; trying insecure HTTP directly to avoid Edge 530: ${backend}`);
        backend = backend.replace("https://", "http://")
      }

      const response = await fetch(backend, fetchInit)
      noteBackendResult(backend, Date.now() - attemptStartedAt, response.status, false)

      if (!failoverOnStatuses.includes(response.status) && response.status < 500 || i === limitedCandidates.length - 1) {
        console.info(`[backend] Candidate ${backend} responded with ${response.status}`);
        return { response, backend: response.url, attempts }
      }

      console.warn(`[failover] Candidate ${backend} returned status ${response.status}; trying next...`);
    } catch (error: any) {
      lastError = error
      noteBackendResult(backend, timeoutMs, undefined, true)
      console.error(`[failover] Candidate ${backend} fetch threw error: ${error?.message || error}; trying next...`);

      if (i === limitedCandidates.length - 1) {
        throw Object.assign(new Error(`All backend candidates failed: ${error?.message || String(error)}`), {
          attempts,
          cause: lastError,
        })
      }
    }

    if (retryJitterMs > 0) {
      await sleep(Math.floor(Math.random() * retryJitterMs))
    }
  }

  throw Object.assign(new Error("No backend candidates available"), { attempts })
}

export function forwardRequestHeaders(contentType = "application/json"): HeadersInit {
  try {
    const incoming = headers()
    const out: Record<string, string> = {
      "Content-Type": contentType,
    }
    incoming.forEach((v, k) => {
      const lowerK = k.toLowerCase()
      // Skip headers that should not be forwarded to the backend
      if (["host", "connection", "content-length", "content-type", "x-forwarded-for", "cf-ray", "cf-connecting-ip", "x-shard-backend-url"].includes(lowerK)) {
        return
      }
      out[k] = v
    })
    return out
  } catch (e) {
    // Return minimal headers if headers() throws (e.g. outside request context during build/edge edge cases)
    return { "Content-Type": contentType }
  }
}
