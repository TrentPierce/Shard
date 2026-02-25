import { detectScoutCapability, type ScoutCapabilityResult } from "./scout-capability"
import * as WebLLM from "./webllm"

// Interface for any scout engine
export interface ScoutEngine {
    init(progressCallback?: (progress: number, text: string) => void): Promise<void>
    generate(prompt: string, options?: any): Promise<{ tokens: number[], text: string, success: boolean, error?: string }>
    reset(): Promise<void>
    mode: "webgpu" | "wasm"
}

// WebGPU Implementation (wraps existing webllm.ts)
class WebGPUScoutEngine implements ScoutEngine {
    mode: "webgpu" = "webgpu" as const

    async init(progressCallback?: (progress: number, text: string) => void) {
        await WebLLM.initWebLLM((p) => {
            progressCallback?.(p.progress, p.text)
        })
    }

    async generate(prompt: string, options?: any) {
        return await WebLLM.generateDraftTokens(prompt, options)
    }

    async reset() {
        await WebLLM.resetWebLLMChat()
    }
}

// WASM Implementation (Placeholder / Future: transformers.js)
class WasmScoutEngine implements ScoutEngine {
    mode: "wasm" = "wasm" as const
    private initialized = false

    async init(progressCallback?: (progress: number, text: string) => void) {
        // Simulate loading
        progressCallback?.(0.1, "Loading WASM runtime...")
        await new Promise(r => setTimeout(r, 500))
        progressCallback?.(0.5, "Loading quantized model (CPU)...")
        await new Promise(r => setTimeout(r, 500))
        progressCallback?.(1.0, "Ready")
        this.initialized = true
        console.log("[WasmScout] Initialized (CPU Mode)")
    }

    async generate(prompt: string, options?: any) {
        if (!this.initialized) throw new Error("WASM Engine not initialized")

        const maxTokens = Math.max(1, Math.min(Number(options?.maxTokens ?? 4), 16))
        const words = prompt
            .replace(/<\|[^>]+?\|>/g, " ")
            .replace(/\s+/g, " ")
            .trim()
            .split(" ")
            .filter(Boolean)
        const tail = words.slice(-maxTokens)
        const text = tail.join(" ").trim()

        return {
            tokens: [],
            text: text.length > 0 ? `${text} ` : "the ",
            success: true,
        }
    }

    async reset() {
        // no-op
    }
}

let activeEngine: ScoutEngine | null = null

export async function initScoutEngine(
    progressCallback?: (progress: number, text: string) => void
): Promise<ScoutCapabilityResult> {
    const report = await detectScoutCapability()
    
    if (report.capability === "webgpu") {
        activeEngine = new WebGPUScoutEngine()
    } else if (report.capability === "wasm") {
        activeEngine = new WasmScoutEngine()
    } else {
        throw new Error("Browser not supported for Scout mode")
    }

    await activeEngine.init(progressCallback)
    return report
}

export function getActiveEngine(): ScoutEngine | null {
    return activeEngine
}

export async function generateDrafts(prompt: string, options?: any) {
    if (!activeEngine) throw new Error("Engine not initialized")
    return await activeEngine.generate(prompt, options)
}
