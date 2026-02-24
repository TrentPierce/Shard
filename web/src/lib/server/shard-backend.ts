import { headers } from "next/headers"

const DEFAULT_BACKEND = "http://35.175.242.222:9091"
const DEFAULT_FALLBACK = "http://35.175.242.222:9091" // Can be a different dedicated verifier

function normalizeUrl(url: string): string {
  return url.trim().replace(/\/$/, "")
}

function parseUrlList(raw?: string | null): string[] {
  if (!raw) return []
  return raw
    .split(/[\n,;\s]+/)
    .map((item) => normalizeUrl(item))
    .filter(Boolean)
}

function dedupe(list: string[]): string[] {
  return Array.from(new Set(list))
}

export function getShardBackendBaseUrls(): string[] {
  const multi = parseUrlList(
    process.env.SHARD_BACKEND_URLS || process.env.NEXT_PUBLIC_SHARD_BACKEND_URLS,
  )
  const single = parseUrlList(
    process.env.SHARD_BACKEND_URL || process.env.NEXT_PUBLIC_SHARD_BACKEND_URL,
  )
  const defaults = [normalizeUrl(DEFAULT_BACKEND)]
  return dedupe([...multi, ...single, ...defaults])
}

export function getFallbackBackendUrls(): string[] {
  const multi = parseUrlList(process.env.SHARD_FALLBACK_URLS)
  const single = parseUrlList(process.env.SHARD_FALLBACK_URL)
  const defaults = [normalizeUrl(DEFAULT_FALLBACK)]
  return dedupe([...multi, ...single, ...defaults])
}

export function getShardBackendBaseUrl(): string {
  return getShardBackendBaseUrls()[0]
}

export function getFallbackBackendUrl(): string {
  return getFallbackBackendUrls()[0]
}

export function shardBackendUrl(path: string, fallback = false): string {
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  const base = fallback ? getFallbackBackendUrl() : getShardBackendBaseUrl()
  return `${base}${cleanPath}`
}

export function shardBackendUrls(path: string, fallback = false): string[] {
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  const bases = fallback ? getFallbackBackendUrls() : getShardBackendBaseUrls()
  return bases.map((base) => `${base}${cleanPath}`)
}

type FetchBackendsOptions = RequestInit & {
  fallback?: boolean
  timeoutMs?: number
  failoverOnStatuses?: number[]
}

export async function fetchWithBackendFailover(
  path: string,
  options: FetchBackendsOptions = {},
): Promise<{ response: Response; backend: string; attempts: string[] }> {
  const {
    fallback = false,
    timeoutMs = 8000,
    failoverOnStatuses = [500, 502, 503, 504],
    ...requestInit
  } = options

  const candidates = shardBackendUrls(path, fallback)
  const attempts: string[] = []
  let lastError: unknown = null

  for (let i = 0; i < candidates.length; i += 1) {
    const backend = candidates[i]
    attempts.push(backend)

    try {
      const response = await fetch(backend, {
        ...requestInit,
        signal: AbortSignal.timeout(timeoutMs),
        cache: "no-store",
      })

      if (!failoverOnStatuses.includes(response.status) || i === candidates.length - 1) {
        return { response, backend, attempts }
      }
    } catch (error) {
      lastError = error
      if (i === candidates.length - 1) {
        throw Object.assign(new Error("All backend candidates failed"), {
          attempts,
          cause: lastError,
        })
      }
    }
  }

  throw Object.assign(new Error("No backend candidates available"), { attempts })
}

export function forwardRequestHeaders(contentType = "application/json"): HeadersInit {
  const incoming = headers()
  const auth = incoming.get("authorization")
  const wallet = incoming.get("x-shard-wallet")
  const inferenceMode = incoming.get("x-shard-inference-mode")

  const out: Record<string, string> = { "Content-Type": contentType }
  if (auth) out.Authorization = auth
  if (wallet) out["X-Shard-Wallet"] = wallet
  if (inferenceMode) out["X-Shard-Inference-Mode"] = inferenceMode
  return out
}
