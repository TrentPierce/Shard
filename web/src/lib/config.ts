/**
 * Shard Configuration
 */

// In production, we want relative URLs so they hit the Next.js API routes (proxies).
// Locally, they will also hit the same relative routes which proxy to the daemon.
export const API_BASE = process.env.NEXT_PUBLIC_API_URL || "" 
export const RUST_BASE = process.env.NEXT_PUBLIC_RUST_URL || "http://localhost:9091"
export const SHARD_BACKEND_BASE = process.env.NEXT_PUBLIC_SHARD_BACKEND_URL || "http://localhost:9091"

/**
 * Whether the browser should prefer local shard mode when a localhost daemon is detected.
 * Default is false so normal visitors contribute as Scout nodes by default.
 */
export const PREFER_LOCAL_SHARD = process.env.NEXT_PUBLIC_PREFER_LOCAL_SHARD === "true"

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
