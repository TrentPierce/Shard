import { headers } from "next/headers"

const DEFAULT_BACKEND = "http://35.175.242.222:9091"
const DEFAULT_FALLBACK = "http://35.175.242.222:8080" // Port 8080 is common on some proxy configs

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

export function shardBackendUrls(path: string = ""): string[] {
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  const multi = parseUrlList(process.env.SHARD_BACKEND_URLS)
  const single = parseUrlList(
    process.env.SHARD_BACKEND_URL || process.env.NEXT_PUBLIC_SHARD_BACKEND_URL,
  )
  const defaults = [normalizeUrl(DEFAULT_BACKEND), normalizeUrl(DEFAULT_FALLBACK)]
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
  } = options

  const candidates = shardBackendUrls(path)
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

      // Automatic HTTPS -> HTTP fallback for raw IP addresses BEFORE the actual fetch
      // Cloudflare Edge Runtime often fails immediately on HTTPS to raw IP.
      const urlObj = new URL(backend)
      const isRawIp = /^\d{1,3}(\.\d{1,3}){3}/.test(urlObj.hostname)
      if (isRawIp && urlObj.protocol === "https:") {
        console.warn(`[backend] Raw IP detected with HTTPS; trying insecure HTTP directly to avoid Edge 530: ${backend}`);
        backend = backend.replace("https://", "http://")
      }

      const response = await fetch(backend, fetchInit)

      if (!failoverOnStatuses.includes(response.status) && response.status < 500 || i === limitedCandidates.length - 1) {
        console.info(`[backend] Candidate ${backend} responded with ${response.status}`);
        return { response, backend: response.url, attempts }
      }

      console.warn(`[failover] Candidate ${backend} returned status ${response.status}; trying next...`);
    } catch (error: any) {
      lastError = error
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
      if (["host", "connection", "content-length", "content-type", "x-forwarded-for", "cf-ray", "cf-connecting-ip"].includes(lowerK)) {
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
