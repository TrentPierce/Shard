import { apiUrl, mergeRuntimeApiHeaders } from "./config"

export interface DraftSubmission {
  work_id: string
  scout_id: string
  lease_id?: string
  draft_text: string
  prompt_context?: string
  timestamp?: number
}

export interface DraftResponse {
  ok: boolean
  detail?: string
  status?: number
  retried?: number
  transient_error?: boolean
  retry_after_ms?: number
}

export interface ScoutConfig {
  maxDraftTokens: number
  temperature: number
  topP: number
  timeoutMs: number
  maxRetries: number
  retryBackoffMs: number
  maxRetryBackoffMs: number
  maxQueueDepth: number
  maxQueueWaitMs: number
  pollTimeoutMs: number
  pollRetries: number
  pollRetryBackoffMs: number
  pollMaxRetryBackoffMs: number
  powRequestTimeoutMs: number
  powRetries: number
  powRetryBackoffMs: number
  powFailureCooldownMs: number
  clientEventTimeoutMs: number
}

export interface SubmitDraftOptions extends Partial<ScoutConfig> {
  promptContext?: string
  leaseId?: string
}

const DEFAULT_CONFIG: ScoutConfig = {
  maxDraftTokens: 4,
  temperature: 0,   // Greedy decoding — maximize match with verifier's argmax
  topP: 1,
  timeoutMs: 15000,
  maxRetries: 2,
  retryBackoffMs: 250,
  maxRetryBackoffMs: 3000,
  maxQueueDepth: 16,
  maxQueueWaitMs: 25000,
  pollTimeoutMs: 1500,
  pollRetries: 2,
  pollRetryBackoffMs: 300,
  pollMaxRetryBackoffMs: 3000,
  powRequestTimeoutMs: 5000,
  powRetries: 1,
  powRetryBackoffMs: 500,
  powFailureCooldownMs: 10000,
  clientEventTimeoutMs: 1200,
}

const SCOUT_ID_KEY = "shard_scout_id"
const SCOUT_SESSION_ID_KEY = "shard_scout_id_session"

let scoutId: string | null = null
let isSubmitting = false
let activeSubmissionAbort: AbortController | null = null
let powVerifiedUntilMs = 0
let powVerificationInFlight: Promise<boolean> | null = null
let powRetryNotBeforeMs = 0
let clientEventMutedUntilMs = 0

type QueueItem = {
  submission: DraftSubmission
  cfg: ScoutConfig
  enqueuedAtMs: number
  resolve: (value: DraftResponse) => void
}

const submissionQueue: QueueItem[] = []
const queuedWorkIds = new Set<string>()
let processingQueue = false

type ScoutClientEventName =
  | "submit_attempt"
  | "submit_success"
  | "submit_http_error"
  | "submit_timeout"
  | "submit_pow_failure"
  | "submit_network_error"
  | "generate_failure"
  | "fallback_draft_used"
  | "runtime_webgpu_ready"
  | "runtime_wasm_fallback"

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function getJitteredBackoff(baseMs: number, attempt: number, capMs: number): number {
  const boundedBase = Math.max(1, Math.floor(baseMs))
  const boundedCap = Math.max(boundedBase, Math.floor(capMs))
  const ceiling = Math.min(boundedCap, boundedBase * Math.pow(2, Math.max(0, attempt)))
  const floor = Math.max(10, Math.floor(ceiling * 0.25))
  return floor + Math.floor(Math.random() * Math.max(1, ceiling - floor + 1))
}

function isTransientDetail(detail: string): boolean {
  const lower = detail.toLowerCase()
  return (
    lower.includes("timeout") ||
    lower.includes("network") ||
    lower.includes("failed to fetch") ||
    lower.includes("temporarily unavailable") ||
    lower.includes("backend returned non-ok") ||
    lower.includes("all backend candidates failed") ||
    lower.includes("http 5")
  )
}

async function fetchWithTimeout(
  input: RequestInfo | URL,
  init: RequestInit,
  timeoutMs: number,
): Promise<Response> {
  const controller = new AbortController()
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs)
  try {
    return await fetch(input, {
      ...init,
      headers: mergeRuntimeApiHeaders(init.headers),
      signal: controller.signal,
    })
  } finally {
    clearTimeout(timeoutId)
  }
}

export async function reportScoutClientEvent(
  event: ScoutClientEventName,
  detail?: string,
  status?: number,
  scoutIdValue?: string,
  options: { bypassMute?: boolean } = {},
): Promise<boolean> {
  if (!options.bypassMute && Date.now() < clientEventMutedUntilMs) {
    return false
  }
  try {
    const response = await fetchWithTimeout(
      apiUrl("/v1/scout/client-event"),
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        keepalive: true,
        body: JSON.stringify({
          scout_id: scoutIdValue ?? getScoutId(),
          event,
          detail: detail?.slice(0, 300),
          status,
        }),
      },
      DEFAULT_CONFIG.clientEventTimeoutMs,
    )
    if (!options.bypassMute && (response.status >= 500 || response.status === 202)) {
      clientEventMutedUntilMs = Date.now() + getJitteredBackoff(10000, 0, 30000)
    }
    return response.ok
  } catch {
    // Best-effort telemetry only. On repeated failures, avoid generating a storm.
    if (!options.bypassMute) {
      clientEventMutedUntilMs = Date.now() + getJitteredBackoff(10000, 0, 30000)
    }
    return false
  }
}

type PowChallengePayload = {
  challenge_bytes_hex: string
  difficulty: number
}

function extractPowChallengePayload(payload: any): PowChallengePayload | null {
  const candidate = payload?.challenge ?? payload
  if (
    candidate &&
    typeof candidate.challenge_bytes_hex === "string" &&
    Number.isFinite(candidate.difficulty) &&
    Number(candidate.difficulty) > 0
  ) {
    return {
      challenge_bytes_hex: String(candidate.challenge_bytes_hex),
      difficulty: Number(candidate.difficulty),
    }
  }
  return null
}

function getHardwareConcurrency(): number {
  if (typeof navigator === "undefined") return 4
  const raw = Number((navigator as any).hardwareConcurrency ?? 4)
  if (!Number.isFinite(raw) || raw <= 0) return 4
  return Math.max(1, Math.floor(raw))
}

function getPowConcurrencyHint(): number {
  // Browser JS PoW solving is materially slower than native threads.
  // Keep a conservative hint so scouts can reliably obtain verification.
  return Math.min(getHardwareConcurrency(), 3)
}

function isMobileDevice(): boolean {
  if (typeof navigator === "undefined") return false
  const ua = navigator.userAgent.toLowerCase()
  return /android|iphone|ipad|ipod|mobile|windows phone/.test(ua)
}

function solvePow(challengeHex: string, difficulty: number): Promise<{ nonce: number; hashHex: string }> {
  return new Promise((resolve, reject) => {
    try {
      const worker = new Worker(new URL("./pow_solver.ts", import.meta.url), { type: "module" })
      worker.onmessage = (event: MessageEvent<any>) => {
        const payload = event.data ?? {}
        if (payload.type === "solved") {
          worker.terminate()
          resolve({ nonce: payload.nonce, hashHex: payload.hashHex })
          return
        }
        if (payload.type === "timeout") {
          worker.terminate()
          reject(new Error(`PoW solve timed out after ${payload.elapsedMs}ms`))
        }
      }
      worker.onerror = (event) => {
        worker.terminate()
        reject(new Error(`PoW worker failed: ${event.message}`))
      }
      worker.postMessage({
        challengeHex,
        difficulty,
        hardwareConcurrency: getHardwareConcurrency(),
      })
    } catch (error) {
      reject(error)
    }
  })
}

async function ensurePowVerifiedForScout(scoutIdValue: string, cfg: ScoutConfig): Promise<boolean> {
  const now = Date.now()
  if (powVerifiedUntilMs > now) {
    return true
  }
  if (powRetryNotBeforeMs > now) {
    const waitMs = powRetryNotBeforeMs - now
    throw new Error(`PoW verification cooling down (${waitMs}ms remaining)`)
  }
  if (powVerificationInFlight) {
    return powVerificationInFlight
  }

  powVerificationInFlight = (async () => {
    for (let attempt = 0; attempt <= cfg.powRetries; attempt += 1) {
      try {
        const challengeUrl = apiUrl(
          `/v1/pow/challenge?peer_id=${encodeURIComponent(scoutIdValue)}&hardware_concurrency=${getPowConcurrencyHint()}&is_mobile=${isMobileDevice()}`,
        )
        const challengeRes = await fetchWithTimeout(
          challengeUrl,
          {
            method: "GET",
            headers: { "Content-Type": "application/json" },
          },
          cfg.powRequestTimeoutMs,
        )
        if (!challengeRes.ok) {
          throw new Error(`PoW challenge failed (HTTP ${challengeRes.status})`)
        }

        const challengeJson = await challengeRes.json()
        const challenge = extractPowChallengePayload(challengeJson)
        if (!challenge) {
          console.warn(
            "Invalid PoW challenge payload:",
            typeof challengeJson === "string"
              ? challengeJson
              : JSON.stringify(challengeJson).slice(0, 400),
          )
          throw new Error("PoW challenge payload is invalid")
        }

        const solved = await solvePow(challenge.challenge_bytes_hex, challenge.difficulty)
        const verifyRes = await fetchWithTimeout(
          apiUrl("/v1/pow/verify"),
          {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              peer_id: scoutIdValue,
              nonce: solved.nonce,
              hash_hex: solved.hashHex,
            }),
          },
          cfg.powRequestTimeoutMs,
        )
        if (!verifyRes.ok) {
          throw new Error(`PoW verify failed (HTTP ${verifyRes.status})`)
        }

        const verifyJson = await verifyRes.json()
        if (!verifyJson?.ok) {
          throw new Error("PoW solution rejected")
        }

        // Daemon default verification TTL is 1 hour.
        powVerifiedUntilMs = Date.now() + 50 * 60 * 1000
        powRetryNotBeforeMs = 0
        return true
      } catch (error) {
        const detail = error instanceof Error ? error.message : String(error)
        const isInvalidChallengePayload = detail.includes("PoW challenge payload is invalid")
        if (attempt >= cfg.powRetries) {
          const cooldownBaseMs = isInvalidChallengePayload
            ? Math.min(cfg.powFailureCooldownMs, 1500)
            : cfg.powFailureCooldownMs
          powRetryNotBeforeMs = Date.now() + getJitteredBackoff(cooldownBaseMs, 0, cooldownBaseMs * 2)
          throw error
        }
        await sleep(getJitteredBackoff(cfg.powRetryBackoffMs, attempt, cfg.powFailureCooldownMs))
      }
    }
    throw new Error("PoW verification retry budget exhausted")
  })()

  try {
    return await powVerificationInFlight
  } finally {
    powVerificationInFlight = null
  }
}

export function getScoutId(): string {
  if (!scoutId) {
    if (typeof window !== "undefined") {
      // Use a per-tab scout id to avoid PoW challenge collisions across tabs.
      // Keeping this scoped to session storage also makes "active scouts" reflect
      // real concurrent browser workers instead of one shared identity.
      scoutId = sessionStorage.getItem(SCOUT_SESSION_ID_KEY)
      if (!scoutId) {
        const seed =
          localStorage.getItem(SCOUT_ID_KEY) ??
          `scout_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`
        localStorage.setItem(SCOUT_ID_KEY, seed)
        scoutId = `${seed}_tab_${Math.random().toString(36).slice(2, 8)}`
        sessionStorage.setItem(SCOUT_SESSION_ID_KEY, scoutId)
      }
    } else {
      scoutId = `scout_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`
    }
  }
  return scoutId
}

function shouldRetrySubmission(result: DraftResponse): boolean {
  if (result.ok) return false
  if (result.transient_error) return true
  if ((result.status ?? 0) === 429 || (result.status ?? 0) === 503) return true
  if ((result.status ?? 0) >= 500) return true
  return isTransientDetail(result.detail ?? "")
}

async function submitDraftOnce(
  submission: DraftSubmission,
  cfg: ScoutConfig
): Promise<DraftResponse> {
  const controller = new AbortController()
  activeSubmissionAbort = controller
  const timeoutId = setTimeout(() => controller.abort(), cfg.timeoutMs)
  try {
    void reportScoutClientEvent("submit_attempt", undefined, undefined, submission.scout_id)
    await ensurePowVerifiedForScout(submission.scout_id, cfg)
    const response = await fetch(apiUrl("/v1/scout/draft"), {
      method: "POST",
      headers: mergeRuntimeApiHeaders({
        "Content-Type": "application/json",
      }),
      body: JSON.stringify(submission),
      signal: controller.signal,
    })
    clearTimeout(timeoutId)

    if (!response.ok) {
      const error = (await response.json().catch(() => ({}))) as DraftResponse
      const detail = error.detail || `HTTP ${response.status}`
      const retryAfterMs = Number(error.retry_after_ms ?? 0) || undefined
      const transientError =
        Boolean(error.transient_error) ||
        response.status === 429 ||
        response.status === 503 ||
        response.status >= 500 ||
        isTransientDetail(detail)
      void reportScoutClientEvent(
        "submit_http_error",
        detail,
        response.status,
        submission.scout_id,
      )
      return {
        ok: false,
        detail,
        status: response.status,
        transient_error: transientError,
        retry_after_ms: retryAfterMs,
      }
    }

    const result = (await response.json().catch(() => ({ ok: false, detail: "Invalid backend response" }))) as DraftResponse
    const transientError = Boolean(result.transient_error) || (!result.ok && isTransientDetail(result.detail ?? ""))
    if (result.ok) {
      void reportScoutClientEvent("submit_success", undefined, response.status, submission.scout_id)
    } else {
      const detail = result.detail || "Draft rejected by verifier/backend"
      void reportScoutClientEvent("submit_http_error", detail, response.status, submission.scout_id)
    }
    return {
      ...result,
      status: response.status,
      transient_error: transientError,
    }
  } catch (error) {
    clearTimeout(timeoutId)
    if (error instanceof Error && error.name === "AbortError") {
      void reportScoutClientEvent("submit_timeout", `timeout ${cfg.timeoutMs}ms`, undefined, submission.scout_id)
      return {
        ok: false,
        detail: `Timeout: verifier did not respond within ${cfg.timeoutMs}ms`,
        transient_error: true,
      }
    }
    const detail = error instanceof Error ? error.message : "Unknown error submitting draft"
    if (detail.toLowerCase().includes("pow")) {
      void reportScoutClientEvent("submit_pow_failure", detail, undefined, submission.scout_id)
    } else {
      void reportScoutClientEvent("submit_network_error", detail, undefined, submission.scout_id)
    }
    return {
      ok: false,
      detail,
      transient_error: isTransientDetail(detail),
    }
  } finally {
    activeSubmissionAbort = null
  }
}

async function submitWithRetry(
  submission: DraftSubmission,
  cfg: ScoutConfig
): Promise<DraftResponse> {
  let attempt = 0
  while (attempt <= cfg.maxRetries) {
    const result = await submitDraftOnce(submission, cfg)
    if (result.ok) {
      return {
        ...result,
        retried: attempt,
      }
    }
    if (attempt >= cfg.maxRetries || !shouldRetrySubmission(result)) {
      return {
        ...result,
        retried: attempt,
      }
    }
    const serverDelay = Math.max(0, Number(result.retry_after_ms ?? 0))
    const clientDelay = getJitteredBackoff(cfg.retryBackoffMs, attempt, cfg.maxRetryBackoffMs)
    await sleep(Math.max(serverDelay, clientDelay))
    attempt += 1
  }
  return { ok: false, detail: "Draft submission retry budget exhausted", retried: cfg.maxRetries }
}

async function processSubmissionQueue(): Promise<void> {
  if (processingQueue) return
  processingQueue = true
  try {
    while (submissionQueue.length > 0) {
      const next = submissionQueue.shift()
      if (!next) continue

      const queueWaitMs = Date.now() - next.enqueuedAtMs
      if (queueWaitMs > next.cfg.maxQueueWaitMs) {
        queuedWorkIds.delete(next.submission.work_id)
        next.resolve({
          ok: false,
          detail: `Draft dropped after waiting ${queueWaitMs}ms in submission queue`,
          transient_error: true,
        })
        continue
      }

      isSubmitting = true
      try {
        const response = await submitWithRetry(next.submission, next.cfg)
        queuedWorkIds.delete(next.submission.work_id)
        next.resolve(response)
      } catch (error) {
        const detail = error instanceof Error ? error.message : "Unexpected queue processing error"
        queuedWorkIds.delete(next.submission.work_id)
        next.resolve({ ok: false, detail, transient_error: true })
      } finally {
        isSubmitting = false
      }
    }
  } finally {
    processingQueue = false
  }
}

export async function submitDraft(
  workId: string,
  draftText: string,
  options: SubmitDraftOptions = {}
): Promise<DraftResponse> {
  const { promptContext, leaseId, ...config } = options
  const cfg = { ...DEFAULT_CONFIG, ...config }
  if (!workId.trim()) {
    void reportScoutClientEvent("submit_network_error", "work_id_missing")
    return { ok: false, detail: "work_id is required" }
  }
  if (!draftText.trim()) {
    void reportScoutClientEvent("submit_network_error", "draft_text_missing")
    return { ok: false, detail: "draft_text is required" }
  }
  if (queuedWorkIds.has(workId)) {
    void reportScoutClientEvent("submit_network_error", "duplicate_work_id")
    return { ok: false, detail: "Duplicate work_id already queued" }
  }
  const queueDepth = submissionQueue.length + (isSubmitting ? 1 : 0)
  if (queueDepth >= cfg.maxQueueDepth) {
    void reportScoutClientEvent("submit_network_error", "queue_full")
    return { ok: false, detail: "Draft submission queue is full", transient_error: true }
  }

  const submission: DraftSubmission = {
    work_id: workId,
    scout_id: getScoutId(),
    lease_id: leaseId,
    draft_text: draftText,
    prompt_context: promptContext,
    timestamp: Date.now() / 1000,
  }

  queuedWorkIds.add(workId)

  return new Promise<DraftResponse>((resolve) => {
    submissionQueue.push({ submission, cfg, enqueuedAtMs: Date.now(), resolve })
    void processSubmissionQueue()
  })
}

export function isDraftSubmitting(): boolean {
  return isSubmitting || processingQueue
}

export function cancelDraftSubmission(): void {
  activeSubmissionAbort?.abort()
  activeSubmissionAbort = null
  for (const queued of submissionQueue) {
    queued.resolve({ ok: false, detail: "Cancelled" })
    queuedWorkIds.delete(queued.submission.work_id)
  }
  submissionQueue.length = 0
  isSubmitting = false
  processingQueue = false
}

export interface WorkItem {
  work: {
    request_id: string
    prompt?: string
    prompt_context?: string
    max_tokens?: number
    min_tokens?: number
    created_at_ms?: number
    lease_id?: string
    lease_expires_at_ms?: number
    assigned_scout_id?: string
    preferred_endpoint?: string
  } | null
  transient_error?: boolean
  fast_bypass?: boolean
  detail?: string
  retry_after_ms?: number
}

export async function pollForWork(
  scoutIdValue: string,
  config: Partial<ScoutConfig> = {}
): Promise<WorkItem> {
  const cfg = { ...DEFAULT_CONFIG, ...config }
  try {
    await ensurePowVerifiedForScout(scoutIdValue, cfg)
  } catch (error) {
    return {
      work: null,
      transient_error: true,
      detail: error instanceof Error ? error.message : "Failed to complete PoW verification",
    }
  }
  let attempt = 0
  while (attempt <= cfg.pollRetries) {
    const controller = new AbortController()
    const timeoutId = setTimeout(() => controller.abort(), cfg.pollTimeoutMs)
    try {
      const response = await fetch(
        apiUrl(`/v1/scout/work?scout_id=${encodeURIComponent(scoutIdValue)}`),
        {
          method: "GET",
          headers: mergeRuntimeApiHeaders({
            "Content-Type": "application/json",
          }),
          signal: controller.signal,
        }
      )
      clearTimeout(timeoutId)
      if (!response.ok) {
        const payload = (await response.json().catch(() => ({}))) as WorkItem
        const retryAfterMs = Math.max(0, Number(payload.retry_after_ms ?? 0))
        const transientStatus = response.status === 429 || response.status === 503 || response.status >= 500
        if (transientStatus && attempt < cfg.pollRetries) {
          const clientDelay = getJitteredBackoff(cfg.pollRetryBackoffMs, attempt, cfg.pollMaxRetryBackoffMs)
          await sleep(Math.max(retryAfterMs, clientDelay))
          attempt += 1
          continue
        }
        return {
          work: null,
          detail: payload.detail || `HTTP ${response.status}`,
          transient_error: transientStatus,
          retry_after_ms: retryAfterMs || undefined,
        }
      }
      const payload = (await response.json()) as WorkItem
      if (payload.transient_error && attempt < cfg.pollRetries) {
        const serverDelay = Math.max(0, Number(payload.retry_after_ms ?? 0))
        const clientDelay = getJitteredBackoff(cfg.pollRetryBackoffMs, attempt, cfg.pollMaxRetryBackoffMs)
        await sleep(Math.max(serverDelay, clientDelay))
        attempt += 1
        continue
      }
      return payload
    } catch (error) {
      clearTimeout(timeoutId)
      if (attempt >= cfg.pollRetries) {
        const detail =
          error instanceof Error && error.name === "AbortError"
            ? `Timeout waiting for /v1/scout/work (${cfg.pollTimeoutMs}ms)`
            : error instanceof Error
              ? error.message
              : "Unknown polling error"
        return { work: null, transient_error: true, detail }
      }
      await sleep(getJitteredBackoff(cfg.pollRetryBackoffMs, attempt, cfg.pollMaxRetryBackoffMs))
      attempt += 1
    }
  }
  return { work: null, transient_error: true, detail: "Polling retry budget exhausted" }
}

export interface DraftResult {
  success: boolean
  workId: string
  draftText: string
  submitted: boolean
  error?: string
}

export async function generateAndSubmitDraft(
  prompt: string,
  workId: string,
  generateDraftFn: (prompt: string) => Promise<string[]>,
  config: Partial<ScoutConfig> = {}
): Promise<DraftResult> {
  try {
    const tokens = await generateDraftFn(prompt)
    if (tokens.length === 0) {
      return {
        success: false,
        workId,
        draftText: "",
        submitted: false,
        error: "No tokens generated",
      }
    }

    const draftText = tokens.join("")
    const response = await submitDraft(workId, draftText, config)

    return {
      success: response.ok,
      workId,
      draftText,
      submitted: response.ok,
      error: response.detail,
    }
  } catch (error) {
    return {
      success: false,
      workId,
      draftText: "",
      submitted: false,
      error: error instanceof Error ? error.message : "Unknown error",
    }
  }
}
