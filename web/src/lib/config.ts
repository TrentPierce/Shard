/**
 * Shard Configuration
 * 
 * Centralized configuration for environment variables and API endpoints.
 * Uses Next.js environment variables with sensible fallbacks for development.
 * 
 * On Vercel: uses relative "/api" prefix so vercel.json rewrites proxy
 *            requests to the EC2 backend.
 * Locally:   hits http://127.0.0.1:9091 directly.
 */

// ─── API Configuration ──────────────────────────────────────────────────────

/**
 * Detect whether we're running on a deployed environment (Vercel).
 * When deployed, we use relative "/api" paths so the vercel.json
 * rewrites can proxy them to the EC2 backend.
 */
function isDeployed(): boolean {
  if (typeof window === "undefined") return false
  const host = window.location.hostname
  return (
    host !== "localhost" &&
    host !== "127.0.0.1" &&
    host !== "0.0.0.0"
  )
}

/**
 * Base URL for the Shard API.
 * 
 * Priority:
 * 1. NEXT_PUBLIC_API_URL env var (explicit override)
 * 2. "/api" relative prefix when deployed (Vercel rewrites to EC2)
 * 3. "http://127.0.0.1:9091" for local development
 */
export const API_BASE = process.env.NEXT_PUBLIC_API_URL || ""

// Rust URL is usually internal (Python -> Rust), but if accessed from browser, use relative + port logic if needed
export const RUST_BASE = process.env.NEXT_PUBLIC_RUST_URL || "http://127.0.0.1:9091"

/**
 * Whether the browser should prefer local shard mode when a localhost daemon is detected.
 * Default is false so normal visitors contribute as Scout nodes by default.
 */
export const PREFER_LOCAL_SHARD = process.env.NEXT_PUBLIC_PREFER_LOCAL_SHARD === "true"

/**
 * Get the API URL for the Shard API.
 * 
 * On Vercel: returns "https://35.175.242.222.nip.io/health", etc.
 * Locally:   returns "http://127.0.0.1:9091/health", etc.
 */
export function apiUrl(path: string = "/v1"): string {
  const cleanPath = path.startsWith("/") ? path : `/${path}`

  // 1. If an explicit env var is set (and not just the relative "/api"), use it
  if (API_BASE && API_BASE !== "/api") {
    const base = API_BASE.replace(/\/$/, "")
    return `${base}${cleanPath}`
  }

  // 2. On deployed environments (Vercel), force the reliable nip.io backend
  if (typeof window !== "undefined" && isDeployed()) {
    return `https://35.175.242.222.nip.io${cleanPath}`
  }

  // 3. Local development: hit the local Rust daemon directly
  // Use port 9091 which is the control plane port
  return `http://127.0.0.1:9091${cleanPath}`
}

/**
 * Build a full Rust control plane URL from a path.
 */
export function rustUrl(path: string): string {
  const base = RUST_BASE.replace(/\/$/, "")
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  return `${base}${cleanPath}`
}
