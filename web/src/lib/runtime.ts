/**
 * Runtime environment helpers shared across browser modules.
 */

const LOCAL_DAEMON_BASE =
  process.env.NEXT_PUBLIC_LOCAL_DAEMON_URL?.trim() || "http://127.0.0.1:9091"

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__ !== undefined
}

export function isLocalBrowserHost(): boolean {
  if (typeof window === "undefined") return false
  const host = window.location.hostname
  return host === "localhost" || host === "127.0.0.1" || host === "::1"
}

/**
 * Local daemon fallback is only safe by default on local hosts or Tauri.
 * It can be explicitly enabled for non-local HTTP hosts via env override.
 */
export function canUseLocalDaemonFallback(): boolean {
  if (typeof window === "undefined") return false
  if (isTauriRuntime() || isLocalBrowserHost()) return true

  const forceEnable = process.env.NEXT_PUBLIC_ENABLE_LOCAL_DAEMON_FALLBACK === "true"
  return forceEnable && window.location.protocol !== "https:"
}

export function localDaemonUrl(path: string): string {
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  return `${LOCAL_DAEMON_BASE.replace(/\/$/, "")}${cleanPath}`
}

