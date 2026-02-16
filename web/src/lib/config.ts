/**
 * Shard Configuration
 * 
 * Centralized configuration for environment variables and API endpoints.
 * Uses Next.js environment variables with sensible fallbacks for development.
 */

// ─── API Configuration ──────────────────────────────────────────────────────

/**
 * Base URL for the Python Shard API.
 * Configurable via NEXT_PUBLIC_API_URL environment variable.
 * @default "http://127.0.0.1:8000"
 */
export const API_BASE = (typeof window !== 'undefined' && window.location.host)
  ? `${window.location.protocol}//${window.location.host}`
  : (process.env.NEXT_PUBLIC_API_URL || "http://127.0.0.1:8000")

// Rust URL is usually internal (Python -> Rust), but if accessed from browser, use relative + port logic if needed
// For now, keep as-is or make relative if we proxy
export const RUST_BASE = process.env.NEXT_PUBLIC_RUST_URL || "http://127.0.0.1:9091"

/**
 * Get the API URL for the Python Shard API.
 * Returns full URL for API requests.
 */
export function apiUrl(path: string = "/v1"): string {
  const base = API_BASE.replace(/\/$/, "") // Remove trailing slash
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  return `${base}${cleanPath}`
}

/**
 * Build a full Rust control plane URL from a path.
 */
export function rustUrl(path: string): string {
  const base = RUST_BASE.replace(/\/$/, "") // Remove trailing slash
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  return `${base}${cleanPath}`
}
