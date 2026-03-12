import { browserModelManifest } from "./browser-model-manifest"
import {
    buildHashedTextEmbedding,
    DEFAULT_EMBEDDING_DIMENSIONS,
} from "./embedding-core"
import type * as OrtTypes from "onnxruntime-web"

type EmbedBatchRequest = {
    id: number
    type: "embed_batch"
    texts: string[]
}

type EmbedBatchResponse = {
    id: number
    ok: true
    embeddings: number[][]
    backend: "onnx-webnn" | "onnx-wasm" | "hash-fallback"
    dimensions: number
    probeMs?: number
    reason?: string
}

type ErrorResponse = {
    id: number
    ok: false
    error: string
    backend: "hash-fallback"
    dimensions: number
}

type RuntimeSession = {
    ort: typeof OrtTypes
    session: OrtTypes.InferenceSession
    backend: "onnx-webnn" | "onnx-wasm"
    probeMs?: number
    reason?: string
}

const workerScope = self as DedicatedWorkerGlobalScope
const embeddingManifest = browserModelManifest.webnnEmbedding
let runtimeSessionPromise: Promise<RuntimeSession | null> | null = null
let ortModulePromise: Promise<typeof OrtTypes> | null = null

function chunkVector(values: Float32Array, chunkSize: number): Float32Array[] {
    const chunks: Float32Array[] = []
    for (let index = 0; index < values.length; index += chunkSize) {
        const chunk = new Float32Array(chunkSize)
        chunk.set(values.slice(index, index + chunkSize))
        chunks.push(chunk)
    }
    return chunks
}

async function createOrtSession(
    ort: typeof OrtTypes,
    modelUrl: string,
    providers: OrtTypes.InferenceSession.SessionOptions["executionProviders"],
): Promise<OrtTypes.InferenceSession> {
    return await ort.InferenceSession.create(modelUrl, {
        executionProviders: providers,
        graphOptimizationLevel: "all",
    })
}

async function getOrtModule(): Promise<typeof OrtTypes> {
    if (!ortModulePromise) {
        const ortUrl = "https://cdn.jsdelivr.net/npm/onnxruntime-web@1.24.3/dist/ort.all.min.mjs"
        ortModulePromise = import(/* webpackIgnore: true */ ortUrl) as Promise<typeof OrtTypes>
    }
    return await ortModulePromise
}

async function getRuntimeSession(): Promise<RuntimeSession | null> {
    if (!runtimeSessionPromise) {
        runtimeSessionPromise = (async () => {
            const startedAt = performance.now()
            try {
                const ort = await getOrtModule()
                const webnnSession = await createOrtSession(
                    ort,
                    embeddingManifest.modelPath,
                    ["webnn"],
                )
                return {
                    ort,
                    session: webnnSession,
                    backend: "onnx-webnn" as const,
                    probeMs: Math.round((performance.now() - startedAt) * 100) / 100,
                }
            } catch (webnnError: any) {
                try {
                    const ort = await getOrtModule()
                    const wasmSession = await createOrtSession(
                        ort,
                        embeddingManifest.modelPath,
                        ["wasm"],
                    )
                    return {
                        ort,
                        session: wasmSession,
                        backend: "onnx-wasm" as const,
                        probeMs: Math.round((performance.now() - startedAt) * 100) / 100,
                        reason: String(webnnError?.message ?? webnnError ?? "WebNN unavailable"),
                    }
                } catch (wasmError: any) {
                    console.warn("[WebNN embeddings] ORT session init failed", wasmError)
                    return null
                }
            }
        })()
    }
    return runtimeSessionPromise
}

async function embedWithOrt(text: string, runtime: RuntimeSession): Promise<number[]> {
    const hashed = buildHashedTextEmbedding(text, embeddingManifest.embeddingDimensions)
    const chunks = chunkVector(hashed, embeddingManifest.chunkSize)
    const projected: number[] = []

    for (const chunk of chunks) {
        const tensor = new runtime.ort.Tensor("float32", chunk, embeddingManifest.inputShape)
        const outputs = await runtime.session.run({
            [embeddingManifest.inputName]: tensor,
        })
        const data = outputs[embeddingManifest.outputName]?.data
        if (!data) {
            throw new Error("Embedding model returned no output tensor")
        }
        projected.push(...Array.from(data as Float32Array | number[]))
    }

    return projected.slice(0, embeddingManifest.embeddingDimensions)
}

workerScope.onmessage = async (event: MessageEvent<EmbedBatchRequest>) => {
    const request = event.data
    if (!request || request.type !== "embed_batch") {
        return
    }

    try {
        const runtime = await getRuntimeSession()
        if (!runtime) {
            const embeddings = request.texts.map((text) =>
                Array.from(buildHashedTextEmbedding(text, DEFAULT_EMBEDDING_DIMENSIONS)),
            )
            const response: EmbedBatchResponse = {
                id: request.id,
                ok: true,
                embeddings,
                backend: "hash-fallback",
                dimensions: DEFAULT_EMBEDDING_DIMENSIONS,
                reason: "ONNX runtime unavailable in worker",
            }
            workerScope.postMessage(response)
            return
        }

        const embeddings = []
        for (const text of request.texts) {
            embeddings.push(await embedWithOrt(text, runtime))
        }
        const response: EmbedBatchResponse = {
            id: request.id,
            ok: true,
            embeddings,
            backend: runtime.backend,
            dimensions: embeddingManifest.embeddingDimensions,
            probeMs: runtime.probeMs,
            reason: runtime.reason,
        }
        workerScope.postMessage(response)
    } catch (error: any) {
        const response: ErrorResponse = {
            id: request.id,
            ok: false,
            error: String(error?.message ?? error ?? "embedding worker failed"),
            backend: "hash-fallback",
            dimensions: embeddingManifest.embeddingDimensions,
        }
        workerScope.postMessage(response)
    }
}

export {}
