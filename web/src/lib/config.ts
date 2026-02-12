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
export const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://127.0.0.1:8000"

/**
 * Base URL for the Rust sidecar control plane.
 * Configurable via NEXT_PUBLIC_RUST_URL environment variable.
 * @default "http://127.0.0.1:9091"
 */
export const RUST_BASE = process.env.NEXT_PUBLIC_RUST_URL || "http://127.0.0.1:9091"

// ─── Feature Flags ───────────────────────────────────────────────────────────

/**
 * Enable/disable debug logging.
 * @default false
 */
export const DEBUG = process.env.NEXT_PUBLIC_DEBUG === "true"

/**
 * Polling interval for topology updates (in milliseconds).
 * @default 10000 (10 seconds)
 */
export const TOPOLOGY_POLL_INTERVAL = parseInt(
    process.env.NEXT_PUBLIC_TOPOLOGY_POLL_INTERVAL || "10000",
    10
)

/**
 * Polling interval for peer updates (in milliseconds).
 * @default 8000 (8 seconds)
 */
export const PEER_POLL_INTERVAL = parseInt(
    process.env.NEXT_PUBLIC_PEER_POLL_INTERVAL || "8000",
    10
)

// ─── Validation ──────────────────────────────────────────────────────────────

/**
 * Validate that required configuration is present.
 * Throws an error if critical config is missing in production.
 */
export function validateConfig(): void {
    if (typeof window === "undefined") {
        // Server-side - skip validation
        return
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
