/**
 * Scout Capability Detection for Shard
 *
 * Detects the browser's capabilities for running Scout mode:
 * - webgpu: Full WebGPU support (Chrome/Edge)
 * - wasm: WebAssembly fallback for non-WebGPU browsers
 * - unsupported: Browser cannot run Scout
 */

import { useState, useEffect } from "react"

export type ScoutCapability = "webgpu" | "wasm" | "unsupported"

export interface ScoutCapabilityResult {
    capability: ScoutCapability
    webgpu: boolean
    wasm: boolean
    reason?: string
    vendor?: string
}

/**
 * Detect WebGPU support in the current browser.
 */
async function detectWebGPU(): Promise<{ supported: boolean; reason?: string; vendor?: string }> {
    if (typeof navigator === "undefined") {
        return { supported: false, reason: "Not in browser environment" }
    }

    // Check if WebGPU API exists
    if (!("gpu" in navigator)) {
        return { supported: false, reason: "WebGPU not supported by this browser" }
    }

    try {
        const adapter = await (navigator as any).gpu.requestAdapter()
        if (!adapter) {
            return { supported: false, reason: "No WebGPU adapter (hardware/driver issue)" }
        }

        const vendor = adapter.info?.vendor || "Unknown"
        return { supported: true, vendor }
    } catch (error: any) {
        return {
            supported: false,
            reason: `WebGPU initialization failed: ${error?.message ?? error}`,
        }
    }
}

/**
 * Detect WebAssembly support.
 */
function detectWASM(): boolean {
    if (typeof WebAssembly === "undefined") {
        return false
    }

    // Check for WASM threading support (SharedArrayBuffer)
    // This is required for efficient model execution
    try {
        if (typeof SharedArrayBuffer !== "undefined") {
            return true
        }

        // Fallback: check basic WASM support
        const testModule = new WebAssembly.Module(
            Uint8Array.of(0x0, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00)
        )
        return testModule instanceof WebAssembly.Module
    } catch {
        return false
    }
}

/**
 * Detect all Scout capabilities.
 * 
 * Priority: WebGPU > WASM > Unsupported
 */
export async function detectScoutCapability(): Promise<ScoutCapabilityResult> {
    // Check WebGPU first (preferred)
    const webgpuResult = await detectWebGPU()

    if (webgpuResult.supported) {
        return {
            capability: "webgpu",
            webgpu: true,
            wasm: true, // WASM is also available
            vendor: webgpuResult.vendor,
        }
    }

    // Check WASM as fallback
    const wasmSupported = detectWASM()

    if (wasmSupported) {
        return {
            capability: "wasm",
            webgpu: false,
            wasm: true,
            reason: webgpuResult.reason, // Why WebGPU failed
        }
    }

    // Neither supported
    return {
        capability: "unsupported",
        webgpu: false,
        wasm: false,
        reason: webgpuResult.reason ?? "WebAssembly not supported",
    }
}

/**
 * Get a human-readable capability label.
 */
export function getCapabilityLabel(capability: ScoutCapability): string {
    switch (capability) {
        case "webgpu":
            return "WebGPU Active"
        case "wasm":
            return "WASM Mode"
        case "unsupported":
            return "Not Supported"
    }
}

/**
 * Get a human-readable reason for the capability.
 */
export function getCapabilityReason(result: ScoutCapabilityResult): string {
    if (result.capability === "webgpu") {
        return `Running on ${result.vendor ?? "WebGPU"}`
    }

    if (result.capability === "wasm") {
        return result.reason ?? "Using WASM fallback mode"
    }

    return result.reason ?? "This browser cannot contribute compute"
}

/**
 * Get the recommended action based on capability.
 */
export function getCapabilityRecommendation(capability: ScoutCapability): string | null {
    switch (capability) {
        case "webgpu":
            return null // No action needed
        case "wasm":
            return "For better performance, try Chrome or Edge"
        case "unsupported":
            return "Try Chrome or Edge for Scout mode"
    }
}

/**
 * Hook to use Scout capability in React components.
 */
export function useScoutCapability() {
    const [capability, setCapability] = useState<ScoutCapabilityResult | null>(null)
    const [loading, setLoading] = useState(true)

    useEffect(() => {
        detectScoutCapability().then((result) => {
            setCapability(result)
            setLoading(false)
        })
    }, [])

    return { capability, loading }
}

// Re-export for convenience
export { useState, useEffect } from "react"
