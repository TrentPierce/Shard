/**
 * Runtime environment helpers shared across browser modules.
 */

const EXPLICIT_LOCAL_DAEMON_BASE = process.env.NEXT_PUBLIC_LOCAL_DAEMON_URL?.trim() || ""
const DEFAULT_LOCAL_DAEMON_BASES = [
  "http://127.0.0.1:9091",
  "http://127.0.0.1:9191",
  "http://localhost:9091",
  "http://localhost:9191",
]

let cachedLocalDaemonBase: string | null = EXPLICIT_LOCAL_DAEMON_BASE || null

function uniqueStrings(values: string[]): string[] {
  return Array.from(new Set(values.map((value) => value.trim()).filter(Boolean)))
}

function normalizeBase(base: string): string {
  return base.replace(/\/$/, "")
}

function localDaemonBaseCandidates(): string[] {
  if (EXPLICIT_LOCAL_DAEMON_BASE) {
    return [normalizeBase(EXPLICIT_LOCAL_DAEMON_BASE)]
  }
  return uniqueStrings(DEFAULT_LOCAL_DAEMON_BASES.map(normalizeBase))
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__ !== undefined
}

export function isLocalBrowserHost(): boolean {
  if (typeof window === "undefined") return false
  const host = window.location.hostname
  return host === "localhost" || host === "127.0.0.1" || host === "::1"
}

/**
 * Direct browser access to a local daemon is only safe by default in Tauri.
 * Regular browser sessions should use same-origin /api routes unless explicitly enabled.
 */
export function canUseLocalDaemonFallback(): boolean {
  if (typeof window === "undefined") return false
  if (isTauriRuntime()) return true

  const forceEnable = process.env.NEXT_PUBLIC_ENABLE_LOCAL_DAEMON_FALLBACK === "true"
  return forceEnable && window.location.protocol !== "https:"
}

export async function getPreferredLocalDaemonBase(): Promise<string | null> {
  if (!canUseLocalDaemonFallback()) {
    return null
  }
  if (cachedLocalDaemonBase) {
    return cachedLocalDaemonBase
  }

  for (const base of localDaemonBaseCandidates()) {
    try {
      const res = await fetch(`${base}/health`, {
        method: "GET",
        cache: "no-store",
        signal: AbortSignal.timeout(1200),
      })
      if (!res.ok) continue
      cachedLocalDaemonBase = base
      return base
    } catch {
      // Try the next candidate.
    }
  }

  return null
}

export function localDaemonUrls(path: string, preferredBase?: string | null): string[] {
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  const bases = uniqueStrings([
    preferredBase ? normalizeBase(preferredBase) : "",
    ...localDaemonBaseCandidates(),
  ])
  return bases.map((base) => `${base}${cleanPath}`)
}

export function localDaemonUrl(path: string, preferredBase?: string | null): string {
  return localDaemonUrls(path, preferredBase)[0]
}
