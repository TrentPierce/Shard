export type BrowserModelBackend = "webgpu" | "webnn"

export type WebGpuModelVariant = {
    logicalId: string
    backend: "webgpu"
    primaryModelId: string
    fallbackModelId: string
    mobileModelId: string
    mobileFallbackModelId: string
    contextWindowSize: number
}

export type WebNnModelVariant = {
    logicalId: string
    backend: "webnn"
    modelPath: string
    inputName: string
    outputName: string
    inputShape: [number, number, number, number]
    chunkSize: number
    embeddingDimensions: number
}

export type BrowserModelManifest = {
    draft: WebGpuModelVariant
    qwenDraft: WebGpuModelVariant
    browserChat: WebGpuModelVariant
    webnnEmbedding: WebNnModelVariant
}

const CONTEXT_WINDOW_SIZE = 512

export const browserModelManifest: BrowserModelManifest = {
    draft: {
        logicalId: "llama-browser-draft",
        backend: "webgpu",
        primaryModelId: "Llama-3.2-1B-Instruct-q4f16_1-MLC",
        fallbackModelId: "TinyLlama-1.1B-Chat-v1.0-q4f32_1-MLC",
        mobileModelId: "Llama-3.2-1B-Instruct-q4f16_1-MLC",
        mobileFallbackModelId: "TinyLlama-1.1B-Chat-v1.0-q4f16_1-MLC",
        contextWindowSize: CONTEXT_WINDOW_SIZE,
    },
    qwenDraft: {
        logicalId: "qwen-browser-draft",
        backend: "webgpu",
        primaryModelId: "Qwen3-0.6B-q4f16_1-MLC",
        fallbackModelId: "Qwen2.5-0.5B-Instruct-q4f16_1-MLC",
        mobileModelId: "Qwen3-0.6B-q4f16_1-MLC",
        mobileFallbackModelId: "Qwen2.5-0.5B-Instruct-q4f16_1-MLC",
        contextWindowSize: CONTEXT_WINDOW_SIZE,
    },
    browserChat: {
        logicalId: "llama-browser-chat",
        backend: "webgpu",
        primaryModelId: "Llama-3.2-1B-Instruct-q4f16_1-MLC",
        fallbackModelId: "TinyLlama-1.1B-Chat-v1.0-q4f32_1-MLC",
        mobileModelId: "Llama-3.2-1B-Instruct-q4f16_1-MLC",
        mobileFallbackModelId: "TinyLlama-1.1B-Chat-v1.0-q4f16_1-MLC",
        contextWindowSize: CONTEXT_WINDOW_SIZE,
    },
    webnnEmbedding: {
        logicalId: "webnn-embedding-probe",
        backend: "webnn",
        modelPath: "/models/webnn/identity.onnx",
        inputName: "x",
        outputName: "y",
        inputShape: [1, 1, 2, 2],
        chunkSize: 4,
        embeddingDimensions: 96,
    },
}

export function getBrowserDraftManifest(preset?: string | null): WebGpuModelVariant {
    const normalized = String(preset || "").trim().toLowerCase()
    return normalized === "qwen" || normalized === "qwen3" || normalized === "qwen-0.6b"
        ? browserModelManifest.qwenDraft
        : browserModelManifest.draft
}
