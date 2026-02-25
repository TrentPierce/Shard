import { apiUrl } from "./config"

export interface DraftSubmission {
  work_id: string
  scout_id: string
  draft_text: string
  prompt_context?: string
  timestamp?: number
}

export interface DraftResponse {
  ok: boolean
  detail?: string
  status?: number
  retried?: number
}

export interface ScoutConfig {
  maxDraftTokens: number
  temperature: number
  topP: number
  timeoutMs: number
  maxRetries: number
  retryBackoffMs: number
  maxQueueDepth: number
  pollTimeoutMs: number
  pollRetries: number
  pollRetryBackoffMs: number
}

export interface SubmitDraftOptions extends Partial<ScoutConfig> {
  promptContext?: string
}

const DEFAULT_CONFIG: ScoutConfig = {
  maxDraftTokens: 4,
  temperature: 0.8,
  topP: 0.9,
  timeoutMs: 800,
  maxRetries: 2,
  retryBackoffMs: 250,
  maxQueueDepth: 16,
  pollTimeoutMs: 1500,
  pollRetries: 2,
  pollRetryBackoffMs: 300,
}

const SCOUT_ID_KEY = "shard_scout_id"
const SCOUT_SESSION_ID_KEY = "shard_scout_id_session"

let scoutId: string | null = null
let isSubmitting = false
let activeSubmissionAbort: AbortController | null = null
let powVerifiedUntilMs = 0
let powVerificationInFlight: Promise<boolean> | null = null

type QueueItem = {
  submission: DraftSubmission
  cfg: ScoutConfig
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

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

export async function reportScoutClientEvent(
  event: ScoutClientEventName,
  detail?: string,
  status?: number,
  scoutIdValue?: string,
): Promise<void> {
  try {
    await fetch(apiUrl("/v1/scout/client-event"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      keepalive: true,
      body: JSON.stringify({
        scout_id: scoutIdValue ?? getScoutId(),
        event,
        detail: detail?.slice(0, 300),
        status,
      }),
    })
  } catch {
    // Best-effort telemetry only.
  }
}

type PowChallengePayload = {
  challenge_bytes_hex: string
  difficulty: number
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

async function ensurePowVerifiedForScout(scoutIdValue: string): Promise<boolean> {
  const now = Date.now()
  if (powVerifiedUntilMs > now) {
    return true
  }
  if (powVerificationInFlight) {
    return powVerificationInFlight
  }

  powVerificationInFlight = (async () => {
    const challengeUrl = apiUrl(
      `/v1/pow/challenge?peer_id=${encodeURIComponent(scoutIdValue)}&hardware_concurrency=${getPowConcurrencyHint()}&is_mobile=${isMobileDevice()}`,
    )
    const challengeRes = await fetch(challengeUrl, {
      method: "GET",
      headers: { "Content-Type": "application/json" },
    })
    if (!challengeRes.ok) {
      throw new Error(`PoW challenge failed (HTTP ${challengeRes.status})`)
    }

    const challengeJson = await challengeRes.json()
    const challenge = challengeJson?.challenge as PowChallengePayload | undefined
    if (!challenge?.challenge_bytes_hex || !challenge?.difficulty) {
      throw new Error("PoW challenge payload is invalid")
    }

    const solved = await solvePow(challenge.challenge_bytes_hex, challenge.difficulty)
    const verifyRes = await fetch(apiUrl("/v1/pow/verify"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        peer_id: scoutIdValue,
        nonce: solved.nonce,
        hash_hex: solved.hashHex,
      }),
    })
    if (!verifyRes.ok) {
      throw new Error(`PoW verify failed (HTTP ${verifyRes.status})`)
    }

    const verifyJson = await verifyRes.json()
    if (!verifyJson?.ok) {
      throw new Error("PoW solution rejected")
    }

    // Daemon default verification TTL is 1 hour.
    powVerifiedUntilMs = Date.now() + 50 * 60 * 1000
    return true
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
  if ((result.status ?? 0) >= 500) return true
  const detail = (result.detail ?? "").toLowerCase()
  return detail.includes("timeout") || detail.includes("network") || detail.includes("failed to fetch")
}

async function submitDraftOnce(
  submission: DraftSubmission,
  cfg: ScoutConfig
): Promise<DraftResponse> {
  const controller = new AbortController()
  activeSubmissionAbort = controller
  const timeoutId = setTimeout(() => controller.abort(), cfg.timeoutMs)
  try {
    await reportScoutClientEvent("submit_attempt", undefined, undefined, submission.scout_id)
    await ensurePowVerifiedForScout(submission.scout_id)
    const response = await fetch(apiUrl("/v1/scout/draft"), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(submission),
      signal: controller.signal,
    })
    clearTimeout(timeoutId)

    if (!response.ok) {
      const error = await response.json().catch(() => ({ detail: "Unknown error" }))
      await reportScoutClientEvent(
        "submit_http_error",
        error.detail || `HTTP ${response.status}`,
        response.status,
        submission.scout_id,
      )
      return {
        ok: false,
        detail: error.detail || `HTTP ${response.status}`,
        status: response.status,
      }
    }

    const result = (await response.json()) as DraftResponse
    await reportScoutClientEvent("submit_success", undefined, response.status, submission.scout_id)
    return {
      ...result,
      status: response.status,
    }
  } catch (error) {
    clearTimeout(timeoutId)
    if (error instanceof Error && error.name === "AbortError") {
      void reportScoutClientEvent("submit_timeout", `timeout ${cfg.timeoutMs}ms`, undefined, submission.scout_id)
      return {
        ok: false,
        detail: `Timeout: verifier did not respond within ${cfg.timeoutMs}ms`,
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
    await sleep(cfg.retryBackoffMs * (attempt + 1))
    attempt += 1
  }
  return { ok: false, detail: "Draft submission retry budget exhausted", retried: cfg.maxRetries }
}

async function processSubmissionQueue(): Promise<void> {
  if (processingQueue) return
  processingQueue = true
  while (submissionQueue.length > 0) {
    const next = submissionQueue.shift()
    if (!next) continue
    isSubmitting = true
    const response = await submitWithRetry(next.submission, next.cfg)
    isSubmitting = false
    queuedWorkIds.delete(next.submission.work_id)
    next.resolve(response)
  }
  processingQueue = false
}

export async function submitDraft(
  workId: string,
  draftText: string,
  options: SubmitDraftOptions = {}
): Promise<DraftResponse> {
  const { promptContext, ...config } = options
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
  if (submissionQueue.length >= cfg.maxQueueDepth) {
    void reportScoutClientEvent("submit_network_error", "queue_full")
    return { ok: false, detail: "Draft submission queue is full" }
  }

  const submission: DraftSubmission = {
    work_id: workId,
    scout_id: getScoutId(),
    draft_text: draftText,
    prompt_context: promptContext,
    timestamp: Date.now() / 1000,
  }

  queuedWorkIds.add(workId)

  return new Promise<DraftResponse>((resolve) => {
    submissionQueue.push({ submission, cfg, resolve })
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
  } | null
  transient_error?: boolean
  detail?: string
}

export async function pollForWork(
  scoutIdValue: string,
  config: Partial<ScoutConfig> = {}
): Promise<WorkItem> {
  const cfg = { ...DEFAULT_CONFIG, ...config }
  try {
    await ensurePowVerifiedForScout(scoutIdValue)
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
          headers: {
            "Content-Type": "application/json",
          },
          signal: controller.signal,
        }
      )
      clearTimeout(timeoutId)
      if (!response.ok) {
        if (response.status >= 500 && attempt < cfg.pollRetries) {
          await sleep(cfg.pollRetryBackoffMs * (attempt + 1))
          attempt += 1
          continue
        }
        return { work: null, detail: `HTTP ${response.status}` }
      }
      return (await response.json()) as WorkItem
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
      await sleep(cfg.pollRetryBackoffMs * (attempt + 1))
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
