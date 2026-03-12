import type { ChatMessage } from "./api"
import { browserModelManifest } from "./browser-model-manifest"
import {
    buildHashedTextEmbedding,
    cosineSimilarity,
} from "./embedding-core"

export type EmbeddingBackend = "onnx-webnn" | "onnx-wasm" | "hash-fallback" | "unsupported"

export type SemanticRelevanceResult = {
    backend: EmbeddingBackend
    scores: number[]
    probeMs?: number
    dimensions: number
    reason?: string
}

type WorkerSuccessResponse = {
    id: number
    ok: true
    embeddings: number[][]
    backend: "onnx-webnn" | "onnx-wasm" | "hash-fallback"
    dimensions: number
    probeMs?: number
    reason?: string
}

type WorkerErrorResponse = {
    id: number
    ok: false
    error: string
    backend: "hash-fallback"
    dimensions: number
}

let workerInstance: Worker | null = null
let requestCounter = 0
const pendingRequests = new Map<
    number,
    {
        resolve: (value: WorkerSuccessResponse) => void
        reject: (error: Error) => void
    }
>()

function localEmbeddings(
    texts: string[],
    dimensions = browserModelManifest.webnnEmbedding.embeddingDimensions,
): number[][] {
    const resolvedDimensions = dimensions || browserModelManifest.webnnEmbedding.embeddingDimensions
    return texts.map((text) => Array.from(buildHashedTextEmbedding(text, resolvedDimensions)))
}

function getEmbeddingWorker(): Worker | null {
    if (typeof window === "undefined" || typeof Worker === "undefined") {
        return null
    }
    if (workerInstance) {
        return workerInstance
    }
    try {
        const worker = new Worker(new URL("./webnn-embeddings-worker.ts", import.meta.url), {
            type: "module",
        })
        worker.onmessage = (event: MessageEvent<WorkerSuccessResponse | WorkerErrorResponse>) => {
            const message = event.data
            const pending = pendingRequests.get(message.id)
            if (!pending) {
                return
            }
            pendingRequests.delete(message.id)
            if (message.ok) {
                pending.resolve(message)
            } else {
                pending.reject(new Error(message.error))
            }
        }
        worker.onerror = (event) => {
            for (const [, pending] of pendingRequests) {
                pending.reject(new Error(event.message || "embedding worker failed"))
            }
            pendingRequests.clear()
            workerInstance = null
        }
        workerInstance = worker
        return worker
    } catch {
        return null
    }
}

async function embedTexts(texts: string[]): Promise<{
    embeddings: number[][]
    backend: EmbeddingBackend
    dimensions: number
    probeMs?: number
    reason?: string
}> {
    const worker = getEmbeddingWorker()
    if (!worker) {
        return {
            embeddings: localEmbeddings(texts),
            backend: typeof window === "undefined" ? "unsupported" : "hash-fallback",
            dimensions: browserModelManifest.webnnEmbedding.embeddingDimensions,
            reason: typeof window === "undefined" ? "No browser worker runtime" : "Worker unavailable",
        }
    }

    const id = ++requestCounter
    const response = await new Promise<WorkerSuccessResponse>((resolve, reject) => {
        pendingRequests.set(id, { resolve, reject })
        worker.postMessage({
            id,
            type: "embed_batch",
            texts,
        })
    }).catch(() => null)

    if (!response) {
        return {
            embeddings: localEmbeddings(texts),
            backend: "hash-fallback",
            dimensions: browserModelManifest.webnnEmbedding.embeddingDimensions,
            reason: "Worker embedding path failed",
        }
    }

    return {
        embeddings: response.embeddings,
        backend: response.backend,
        dimensions: response.dimensions,
        probeMs: response.probeMs,
        reason: response.reason,
    }
}

export async function rankMessagesBySemanticRelevance(
    messages: ChatMessage[],
    focusText: string,
): Promise<SemanticRelevanceResult> {
    if (messages.length === 0) {
        return {
            backend: "unsupported",
            scores: [],
            dimensions: browserModelManifest.webnnEmbedding.embeddingDimensions,
            reason: "No messages to rank",
        }
    }

    const embeddingResult = await embedTexts([focusText, ...messages.map((message) => message.content)])
    const [focusEmbedding, ...messageEmbeddings] = embeddingResult.embeddings
    const scores = messageEmbeddings.map((embedding) =>
        Math.max(0, cosineSimilarity(focusEmbedding, embedding)),
    )

    return {
        backend: embeddingResult.backend,
        scores,
        probeMs: embeddingResult.probeMs,
        dimensions: embeddingResult.dimensions,
        reason: embeddingResult.reason,
    }
}
