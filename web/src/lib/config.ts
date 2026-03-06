/**
 * Shard Configuration
 */

// In production, we want relative URLs so they hit the Next.js API routes (proxies).
// Locally, they will also hit the same relative routes which proxy to the daemon.
// In Tauri, there is no server-side proxy, so we must hit the local daemon directly.
const isTauri = typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__ !== undefined;
export const API_BASE = process.env.NEXT_PUBLIC_API_URL || (isTauri ? "http://localhost:9091" : "")
export const RUST_BASE = process.env.NEXT_PUBLIC_RUST_URL || "http://localhost:9091"
export const SHARD_BACKEND_BASE = process.env.NEXT_PUBLIC_SHARD_BACKEND_URL || "http://localhost:9091"

/**
 * Whether the browser should prefer local shard mode when a localhost daemon is detected.
 * Default is false so normal visitors contribute as Scout nodes by default.
 */
export const PREFER_LOCAL_SHARD = process.env.NEXT_PUBLIC_PREFER_LOCAL_SHARD === "true"

/**
 * Browser layer hosting is an experimental path and should only be enabled when
 * the backend has been explicitly prepared to accept browser-layer sessions.
 */
export const ENABLE_BROWSER_LAYER_HOST =
  process.env.NEXT_PUBLIC_ENABLE_BROWSER_LAYER_HOST === "true"

/**
 * Browser libp2p is optional. The browser scout HTTP work loop is sufficient
 * for normal contribution, so keep browser P2P off unless explicitly testing it.
 */
export const ENABLE_BROWSER_P2P =
  process.env.NEXT_PUBLIC_ENABLE_BROWSER_P2P === "true"

export function apiUrl(path: string = "/v1"): string {
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  // If API_BASE is empty, it results in a relative path like "/api/v1/..."
  const base = API_BASE.replace(/\/$/, "")
  
  // Ensure we use the /api prefix for relative routes if no absolute URL is provided
  if (!base && !cleanPath.startsWith("/api")) {
    return `/api${cleanPath}`
  }
  
  return `${base}${cleanPath}`
}

export function rustUrl(path: string): string {
  const base = RUST_BASE.replace(/\/$/, "")
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  return `${base}${cleanPath}`
}
