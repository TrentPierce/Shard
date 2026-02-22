/**
 * Shard Configuration
 */

export const API_BASE = process.env.NEXT_PUBLIC_API_URL || "/api"
export const RUST_BASE = process.env.NEXT_PUBLIC_RUST_URL || "http://127.0.0.1:9091"
export const SHARD_BACKEND_BASE = process.env.NEXT_PUBLIC_SHARD_BACKEND_URL || "http://35.175.242.222:9091"

/**
 * Whether the browser should prefer local shard mode when a localhost daemon is detected.
 * Default is false so normal visitors contribute as Scout nodes by default.
 */
export const PREFER_LOCAL_SHARD = process.env.NEXT_PUBLIC_PREFER_LOCAL_SHARD === "true"

export function apiUrl(path: string = "/v1"): string {
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  const base = API_BASE.replace(/\/$/, "")
  return `${base}${cleanPath}`
}

export function rustUrl(path: string): string {
  const base = RUST_BASE.replace(/\/$/, "")
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  return `${base}${cleanPath}`
}
