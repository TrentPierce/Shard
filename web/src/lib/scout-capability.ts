/**
 * Scout Capability Detection for Shard
 *
 * The current interactive scout runtime still uses WebGPU/WebLLM, but we also
 * detect whether the browser exposes a viable low-power WebNN path so the
 * network can classify background-capable contributors before the engine swap.
 */

import { useState, useEffect } from "react"

export type ScoutCapability = "webgpu" | "wasm" | "unsupported"
export type BackgroundAcceleration = "webnn" | "webgpu" | "wasm" | "unsupported"
export type WebNNWarmState = "warm" | "cold" | "unsupported" | "failed"

export interface ScoutCapabilityResult {
    capability: ScoutCapability
    webgpu: boolean
    wasm: boolean
    webnn: boolean
    lowPowerEligible: boolean
    backgroundAcceleration: BackgroundAcceleration
    reason?: string
    vendor?: string
    webnnReason?: string
    webnnProbeMs?: number
    webnnWarmState?: WebNNWarmState
}

type WebNNProbeResult = {
    available: boolean
    reason?: string
    probeMs?: number
    warmState: WebNNWarmState
}

const WEBNN_WARM_STATE_KEY = "shard:webnn-warm-state:v1"
let capabilityPromise: Promise<ScoutCapabilityResult> | null = null

function readWebNNWarmState(): boolean {
    if (typeof window === "undefined") return false
    try {
        return window.sessionStorage.getItem(WEBNN_WARM_STATE_KEY) === "warm"
    } catch {
        return false
    }
}

function writeWebNNWarmState(warm: boolean): void {
    if (typeof window === "undefined") return
    try {
        if (warm) {
            window.sessionStorage.setItem(WEBNN_WARM_STATE_KEY, "warm")
        } else {
            window.sessionStorage.removeItem(WEBNN_WARM_STATE_KEY)
        }
    } catch {
        // Best effort only.
    }
}

/**
 * Detect WebGPU support in the current browser.
 */
async function detectWebGPU(): Promise<{ supported: boolean; reason?: string; vendor?: string }> {
    if (typeof navigator === "undefined") {
        return { supported: false, reason: "Not in browser environment" }
    }

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

async function runTinyWebNNGraphProbe(context: any): Promise<void> {
    const GraphBuilder = (globalThis as any).MLGraphBuilder
    if (
        typeof GraphBuilder !== "function" ||
        typeof context?.createTensor !== "function" ||
        typeof context?.writeTensor !== "function" ||
        typeof context?.dispatch !== "function" ||
        typeof context?.readTensor !== "function"
    ) {
        return
    }

    const descriptor = { dataType: "float32", shape: [2, 2] }
    const builder = new GraphBuilder(context)
    const lhs = builder.input("lhs", descriptor)
    const rhs = builder.input("rhs", descriptor)
    const output = builder.add(lhs, rhs)
    const graph = await builder.build({ output })

    const lhsTensor = await context.createTensor(descriptor)
    const rhsTensor = await context.createTensor(descriptor)
    const outputTensor = await context.createTensor(descriptor)

    try {
        context.writeTensor(lhsTensor, new Float32Array([1, 2, 3, 4]))
        context.writeTensor(rhsTensor, new Float32Array([0.5, 0.5, 0.5, 0.5]))
        context.dispatch(graph, { lhs: lhsTensor, rhs: rhsTensor }, { output: outputTensor })
        await context.readTensor(outputTensor)
    } finally {
        lhsTensor?.destroy?.()
        rhsTensor?.destroy?.()
        outputTensor?.destroy?.()
        graph?.destroy?.()
    }
}

/**
 * Detect whether the browser exposes a low-power WebNN path and actively probe it.
 */
async function detectWebNN(): Promise<WebNNProbeResult> {
    if (typeof navigator === "undefined") {
        return { available: false, reason: "Not in browser environment", warmState: "unsupported" }
    }

    const navigatorAny = navigator as any
    const ml = navigatorAny.ml
    if (!ml || typeof ml.createContext !== "function") {
        return { available: false, reason: "WebNN not supported by this browser", warmState: "unsupported" }
    }

    const warmBeforeProbe = readWebNNWarmState()
    const startedAt = performance.now()
    let context: any = null
    try {
        context = await ml.createContext({
            deviceType: "npu",
            powerPreference: "low-power",
        })
        if (!context) {
            return { available: false, reason: "WebNN context unavailable", warmState: "failed" }
        }

        await runTinyWebNNGraphProbe(context)
        const probeMs = Math.round((performance.now() - startedAt) * 100) / 100
        writeWebNNWarmState(true)
        return {
            available: true,
            probeMs,
            warmState: warmBeforeProbe ? "warm" : "cold",
        }
    } catch (error: any) {
        return {
            available: false,
            reason: `WebNN low-power probe failed: ${error?.message ?? error}`,
            warmState: "failed",
        }
    } finally {
        context?.destroy?.()
    }
}

/**
 * Detect WebAssembly support.
 */
function detectWASM(): boolean {
    if (typeof WebAssembly === "undefined") {
        return false
    }

    try {
        if (typeof SharedArrayBuffer !== "undefined") {
            return true
        }

        const testModule = new WebAssembly.Module(
            Uint8Array.of(0x0, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00)
        )
        return testModule instanceof WebAssembly.Module
    } catch {
        return false
    }
}

/**
 * Detect all scout capabilities.
 *
 * Current interactive priority: WebGPU > WASM > Unsupported
 * Background power-aware hinting: WebNN > WebGPU > WASM > Unsupported
 */
export async function detectScoutCapability(): Promise<ScoutCapabilityResult> {
    if (capabilityPromise) {
        return capabilityPromise
    }

    capabilityPromise = (async () => {
        const [webgpuResult, webnnResult] = await Promise.all([detectWebGPU(), detectWebNN()])
        const wasmSupported = detectWASM()
        const backgroundAcceleration: BackgroundAcceleration = webnnResult.available
            ? "webnn"
            : webgpuResult.supported
                ? "webgpu"
                : wasmSupported
                    ? "wasm"
                    : "unsupported"

        if (webgpuResult.supported) {
            return {
                capability: "webgpu",
                webgpu: true,
                wasm: true,
                webnn: webnnResult.available,
                lowPowerEligible: webnnResult.available,
                backgroundAcceleration,
                vendor: webgpuResult.vendor,
                webnnReason: webnnResult.reason,
                webnnProbeMs: webnnResult.probeMs,
                webnnWarmState: webnnResult.warmState,
            }
        }

        if (wasmSupported) {
            return {
                capability: "wasm",
                webgpu: false,
                wasm: true,
                webnn: webnnResult.available,
                lowPowerEligible: webnnResult.available,
                backgroundAcceleration,
                reason: webgpuResult.reason,
                webnnReason: webnnResult.reason,
                webnnProbeMs: webnnResult.probeMs,
                webnnWarmState: webnnResult.warmState,
            }
        }

        return {
            capability: "unsupported",
            webgpu: false,
            wasm: false,
            webnn: webnnResult.available,
            lowPowerEligible: webnnResult.available,
            backgroundAcceleration,
            reason: webgpuResult.reason ?? "WebAssembly not supported",
            webnnReason: webnnResult.reason,
            webnnProbeMs: webnnResult.probeMs,
            webnnWarmState: webnnResult.warmState,
        }
    })()

    return capabilityPromise
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
    const webnnSuffix = result.webnn
        ? ` Low-power WebNN path detected (${result.webnnWarmState ?? "cold"}${typeof result.webnnProbeMs === "number" ? `, ${result.webnnProbeMs.toFixed(1)}ms probe` : ""}).`
        : ""

    if (result.capability === "webgpu") {
        return `Running on ${result.vendor ?? "WebGPU"}.${webnnSuffix}`.trim()
    }

    if (result.capability === "wasm") {
        const base = result.reason ?? "Using WASM fallback mode"
        return `${base}.${webnnSuffix}`.trim()
    }

    return `${result.reason ?? "This browser cannot contribute compute"}.${webnnSuffix}`.trim()
}

/**
 * Get the recommended action based on capability.
 */
export function getCapabilityRecommendation(
    input: ScoutCapability | ScoutCapabilityResult
): string | null {
    const capability = typeof input === "string" ? input : input.capability
    const webnnAvailable = typeof input === "string" ? false : input.webnn

    switch (capability) {
        case "webgpu":
            return webnnAvailable
                ? "WebGPU is best for bursty draft work; WebNN can later serve the low-power background lane."
                : null
        case "wasm":
            return webnnAvailable
                ? "A low-power WebNN path is detected, but the NPU runtime is not enabled yet. Chrome or Edge still gives the current best scout path."
                : "For better performance, try Chrome or Edge"
        case "unsupported":
            return webnnAvailable
                ? "This browser exposes WebNN, and Shard can now use the ONNX/WebNN embeddings worker path for low-risk background compaction tasks."
                : "Try Chrome or Edge for Scout mode"
    }
}

/**
 * Hook to use scout capability in React components.
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

export { useState, useEffect } from "react"
