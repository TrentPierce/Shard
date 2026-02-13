/**
 * WebLLM Engine Wrapper for Shard Scout Node
 *
 * - WebGPU availability detection
 * - MLCEngine initialization
 * - Draft token generation for speculative decoding
 */

// ─── Imports ───────────────────────────────────────────────────────────────

import {
    CreateMLCEngine,
    type MLCEngine,
    type InitProgressReport,
} from "@mlc-ai/web-llm"

// ─── Types ──────────────────────────────────────────────────────────────────

export type WebGPUStatus = {
    supported: boolean
    reason?: string
    vendor?: string
}

export type ModelProgress = {
    progress: number // 0 to 1
    text: string
    timeElapsed: number
}

export type ProgressCallback = (progress: ModelProgress) => void

export type DraftTokenResult = {
    tokens: number[]
    text: string
    success: boolean
    error?: string
}

export type DraftGenerationOptions = {
    maxTokens?: number
    temperature?: number
    topP?: number
}

// ─── Constants ──────────────────────────────────────────────────────────────

// Small model suitable for draft generation (Llama-3.2-1B-Instruct is ideal)
const DRAFT_MODEL = "Llama-3.2-1B-Instruct-q4f32_1-MLC"

// Nano model for mobile devices (highly quantized for 4GB RAM limit)
const NANO_MODEL = "Llama-3.2-1B-Instruct-q4f16_0-MLC"

// Mobile device memory threshold (4GB in bytes)
const MOBILE_MEMORY_THRESHOLD = 4 * 1024 * 1024 * 1024

// Default generation options for speculative decoding drafts
const DEFAULT_DRAFT_OPTIONS: DraftGenerationOptions = {
    maxTokens: 10,
    temperature: 0.8,
    topP: 0.9,
}

// ─── State ─────────────────────────────────────────────────────────────────

let engine: MLCEngine | null = null
let isLoading = false
let currentModel: string = DRAFT_MODEL

// ─── Mobile Detection ─────────────────────────────────────────────────────────

/**
 * Detect if the current device is a mobile device.
 * 
 * Checks:
 * 1. User agent for mobile patterns
 * 2. Screen size (small screens typically indicate mobile)
 * 3. Available device memory (if available via navigator.deviceMemory)
 * 
 * @returns True if the device appears to be mobile
 */
export function isMobileDevice(): boolean {
    if (typeof navigator === "undefined") {
        return false
    }
    
    const ua = navigator.userAgent.toLowerCase()
    
    // Check user agent patterns
    const mobilePatterns = [
        /android/i,
        /webos/i,
        /iphone/i,
        /ipad/i,
        /ipod/i,
        /blackberry/i,
        /windows phone/i,
        /mobile/i,
    ]
    
    const isMobileUA = mobilePatterns.some(pattern => pattern.test(ua))
    if (isMobileUA) {
        return true
    }
    
    // Check screen size (typical mobile cutoffs)
    if (typeof screen !== "undefined") {
        const maxScreenDim = Math.max(screen.width, screen.height)
        if (maxScreenDim < 768) {
            return true
        }
    }
    
    // Check device memory (if available)
    // @ts-expect-error deviceMemory is not in standard types
    const deviceMemory = navigator.deviceMemory
    if (deviceMemory !== undefined && deviceMemory < 4) {
        return true
    }
    
    return false
}

/**
 * Get the appropriate model for the current device.
 * 
 * Mobile devices use the highly quantized Nano model (q4f16_0)
 * to fit within 4GB RAM constraints.
 * 
 * @returns The model ID to use for this device
 */
export function getModelForDevice(): string {
    if (isMobileDevice()) {
        console.log("[WebLLM] Mobile device detected, using Nano model:", NANO_MODEL)
        return NANO_MODEL
    }
    return DRAFT_MODEL
}

// ─── Functions ──────────────────────────────────────────────────────────────

/**
 * Check if WebGPU is available in the current browser.
 *
 * This follows the detection pattern from MDN and WebGPU documentation:
 * 1. Check navigator.gpu availability
 * 2. Request an adapter to verify hardware support
 * 3. Get vendor information for debugging
 */
export async function checkWebGPUSupport(): Promise<WebGPUStatus> {
    // Check if running in browser environment
    if (typeof navigator === "undefined") {
        return {
            supported: false,
            reason: "Not running in a browser environment",
        }
    }

    // Check if WebGPU API exists
    if (!("gpu" in navigator)) {
        return {
            supported: false,
            reason: "WebGPU not supported by this browser",
        }
    }

    try {
        const adapter = await (navigator as any).gpu.requestAdapter()
        if (!adapter) {
            return {
                supported: false,
                reason: "No WebGPU adapter found (hardware/driver issue)",
            }
        }

        // Get vendor info for debugging
        const vendor = adapter.info?.vendor || "Unknown"

        return {
            supported: true,
            vendor,
        }
    } catch (error: any) {
        return {
            supported: false,
            reason: `WebGPU initialization failed: ${error?.message ?? error}`,
        }
    }
}

/**
 * Initialize the WebLLM engine with the draft model.
 *
 * This loads a small quantized model suitable for generating draft tokens
 * for speculative decoding. The model is loaded via WebGPU and cached in
 * IndexedDB for faster subsequent loads.
 *
 * @param progressCallback - Optional callback for loading progress
 * @returns Promise that resolves when the engine is ready
 */
export async function initWebLLM(
    progressCallback?: ProgressCallback
): Promise<void> {
    if (engine) {
        return // Already initialized
    }

    if (isLoading) {
        throw new Error("WebLLM initialization already in progress")
    }

    // Check WebGPU availability first
    const gpuStatus = await checkWebGPUSupport()
    if (!gpuStatus.supported) {
        throw new Error(
            `WebGPU not available: ${gpuStatus.reason}. Cannot run Scout mode.`
        )
    }

    isLoading = true

    const startTime = performance.now()

    const wrappedCallback = (report: InitProgressReport) => {
        const elapsed = performance.now() - startTime
        const progress: ModelProgress = {
            progress: report.progress,
            text: report.text,
            timeElapsed: elapsed,
        }
        progressCallback?.(progress)
    }

    try {
        // Use device-appropriate model (Nano for mobile, standard for desktop)
        const model = getModelForDevice()
        currentModel = model
        
        // Initialize MLCEngine with the appropriate draft model
        engine = await CreateMLCEngine(
            model,
            {
                initProgressCallback: wrappedCallback,
                logLevel: "INFO",
            },
            {
                // Use a smaller context window for draft generation
                context_window_size: 2048,
            }
        )

        isLoading = false
    } catch (error: any) {
        isLoading = false
        engine = null
        throw new Error(
            `Failed to initialize WebLLM: ${error?.message ?? error}`
        )
    }
}

/**
 * Generate draft tokens for speculative decoding.
 *
 * This function takes a prompt context and generates a short sequence of
 * draft tokens using the small WebLLM model. These tokens are sent to an
 * Shard node for verification as part of the hybrid speculative decoding
 * workflow.
 *
 * @param prompt - The prompt context to generate from
 * @param options - Optional generation parameters
 * @returns Promise containing generated draft tokens
 */
export async function generateDraftTokens(
    prompt: string,
    options?: DraftGenerationOptions
): Promise<DraftTokenResult> {
    if (!engine) {
        throw new Error(
            "WebLLM engine not initialized. Call initWebLLM() first."
        )
    }

    if (isLoading) {
        throw new Error("WebLLM engine is still loading")
    }

    const opts = { ...DEFAULT_DRAFT_OPTIONS, ...options }

    try {
        const response = await engine.chat.completions.create({
            messages: [{ role: "user", content: prompt }],
            max_tokens: opts.maxTokens,
            temperature: opts.temperature,
            top_p: opts.topP,
        })

        const text = response.choices[0]?.message?.content || ""

        // Get the generated text - token extraction happens at the API layer
        // We return the text which will be tokenized by the Shard
        return {
            tokens: [], // Token IDs are handled by the Shard
            text,
            success: true,
        }
    } catch (error: any) {
        return {
            tokens: [],
            text: "",
            success: false,
            error: `Draft generation failed: ${error?.message ?? error}`,
        }
    }
}

/**
 * Check if the WebLLM engine is initialized and ready.
 */
export function isWebLLMReady(): boolean {
    return engine !== null && !isLoading
}

/**
 * Get the current model ID being used.
 */
export function getCurrentModel(): string {
    return currentModel
}

/**
 * Reset the WebLLM engine state.
 *
 * This clears the chat history but keeps the model loaded.
 */
export async function resetWebLLMChat(): Promise<void> {
    if (!engine) return

    try {
        await engine.resetChat(true) // keepStats = true
    } catch (error: any) {
        console.error("Failed to reset WebLLM chat:", error)
    }
}

/**
 * Check if the model is cached in IndexedDB.
 *
 * @param modelId - Optional model ID to check (defaults to draft model)
 */
export async function isModelCached(modelId?: string): Promise<boolean> {
    try {
        const { hasModelInCache } = await import("@mlc-ai/web-llm")
        return await hasModelInCache(modelId || getModelForDevice())
    } catch {
        return false
    }
}

// ─── Wake Lock API (Mobile Scout Mode) ───────────────────────────────────────

let wakeLock: WakeLockSentinel | null = null

/**
 * Request a wake lock to keep the screen awake.
 * 
 * This is useful for mobile Scout mode when background processing
 * is needed. Uses the Wake Lock API.
 * 
 * @returns True if wake lock was acquired
 */
export async function requestWakeLock(): Promise<boolean> {
    if (typeof navigator === "undefined" || !("wakeLock" in navigator)) {
        console.warn("[WebLLM] Wake Lock API not supported")
        return false
    }
    
    try {
        wakeLock = await navigator.wakeLock.request("screen")
        console.log("[WebLLM] Wake lock acquired for mobile Scout mode")
        
        // Handle release automatically when visibility changes
        wakeLock.addEventListener("release", () => {
            console.log("[WebLLM] Wake lock released")
            wakeLock = null
        })
        
        return true
    } catch (error: any) {
        console.error("[WebLLM] Failed to acquire wake lock:", error?.message ?? error)
        return false
    }
}

/**
 * Release the wake lock if held.
 * 
 * Call this when Scout mode is disabled or when background
 * processing is no longer needed.
 */
export async function releaseWakeLock(): Promise<void> {
    if (wakeLock) {
        try {
            await wakeLock.release()
            wakeLock = null
            console.log("[WebLLM] Wake lock manually released")
        } catch (error: any) {
            console.error("[WebLLM] Failed to release wake lock:", error?.message ?? error)
        }
    }
}

/**
 * Check if wake lock is currently held.
 */
export function isWakeLockActive(): boolean {
    return wakeLock !== null
}
