/**
 * Shard Configuration
 * 
 * Centralized configuration for environment variables and API endpoints.
 * Uses Next.js environment variables with sensible fallbacks for development.
 */

// ─── API Configuration ──────────────────────────────────────────────────────

/**
 * Base URL for the Python Oracle API.
 * Configurable via NEXT_PUBLIC_API_URL environment variable.
 * @default "http://127.0.0.1:8000"
 */
export const API_BASE = process.env.NEXT_PUBLIC_API_URL || ""
export const RUST_BASE = process.env.NEXT_PUBLIC_RUST_URL || ""

/**
 * Get the API URL for the Python Oracle API.
 * Returns relative URL for same-origin requests, full URL for absolute requests.
 */
export function apiUrl(path: string = "/v1"): string {
  const base = API_BASE || new URL("/v1").toString()
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  return `${base}${cleanPath}`
}

    // In production, require explicit API URL
    if (process.env.NODE_ENV === "production" && !process.env.NEXT_PUBLIC_API_URL) {
        console.warn(
            "[Config] WARNING: NEXT_PUBLIC_API_URL not set in production. " +
            "Using default localhost:8000 which may not work correctly."
        )
    }

    if (DEBUG) {
        console.log("[Config] API_BASE:", API_BASE)
        console.log("[Config] RUST_BASE:", RUST_BASE)
    }
}

// ─── Utility Functions ───────────────────────────────────────────────────────

/**
 * Build a full API URL from a path.
 * @param path - API path (e.g., "/v1/chat/completions")
 * @returns Full URL
 */
export function apiUrl(path: string): string {
    const base = API_BASE.replace(/\/$/, "") // Remove trailing slash
    const cleanPath = path.startsWith("/") ? path : `/${path}`
    return `${base}${cleanPath}`
}

/**
 * Build a full Rust control plane URL from a path.
 * @param path - Control plane path (e.g., "/health")
 * @returns Full URL
 */
export function rustUrl(path: string): string {
    const base = RUST_BASE.replace(/\/$/, "") // Remove trailing slash
    const cleanPath = path.startsWith("/") ? path : `/${path}`
    return `${base}${cleanPath}`
}
