import {
    buildHashedTextEmbedding,
    DEFAULT_EMBEDDING_DIMENSIONS,
} from "./embedding-core"

type EmbedBatchRequest = {
    id: number
    type: "embed_batch"
    texts: string[]
    dimensions?: number
}

type EmbedBatchResponse = {
    id: number
    ok: true
    embeddings: number[][]
    backend: "webnn-worker" | "hash-fallback"
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

const workerScope = self as DedicatedWorkerGlobalScope
let webnnProbePromise: Promise<{ available: boolean; probeMs?: number; reason?: string }> | null = null

async function runTinyWebNNGraphProbe(context: any): Promise<void> {
    const GraphBuilder = (globalThis as any).MLGraphBuilder
    if (
        typeof GraphBuilder !== "function" ||
        typeof context?.createTensor !== "function" ||
        typeof context?.writeTensor !== "function" ||
        typeof context?.dispatch !== "function" ||
        typeof context?.readTensor !== "function"
    ) {
        throw new Error("WebNN graph primitives unavailable")
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
        context.writeTensor(rhsTensor, new Float32Array([0.25, 0.25, 0.25, 0.25]))
        context.dispatch(graph, { lhs: lhsTensor, rhs: rhsTensor }, { output: outputTensor })
        await context.readTensor(outputTensor)
    } finally {
        lhsTensor?.destroy?.()
        rhsTensor?.destroy?.()
        outputTensor?.destroy?.()
        graph?.destroy?.()
    }
}

async function probeWorkerWebNN(): Promise<{ available: boolean; probeMs?: number; reason?: string }> {
    if (!webnnProbePromise) {
        webnnProbePromise = (async () => {
            const startedAt = performance.now()
            const navigatorAny = (globalThis as any).navigator
            const ml = navigatorAny?.ml
            if (!ml || typeof ml.createContext !== "function") {
                return {
                    available: false,
                    reason: "WebNN unavailable in worker",
                }
            }

            let context: any = null
            try {
                context = await ml.createContext({
                    deviceType: "npu",
                    powerPreference: "low-power",
                })
                if (!context) {
                    return {
                        available: false,
                        reason: "WebNN worker context unavailable",
                    }
                }
                await runTinyWebNNGraphProbe(context)
                return {
                    available: true,
                    probeMs: Math.round((performance.now() - startedAt) * 100) / 100,
                }
            } catch (error: any) {
                return {
                    available: false,
                    reason: String(error?.message ?? error ?? "unknown"),
                }
            } finally {
                context?.destroy?.()
            }
        })()
    }
    return webnnProbePromise
}

workerScope.onmessage = async (event: MessageEvent<EmbedBatchRequest>) => {
    const request = event.data
    if (!request || request.type !== "embed_batch") {
        return
    }

    const dimensions = request.dimensions ?? DEFAULT_EMBEDDING_DIMENSIONS
    try {
        const webnn = await probeWorkerWebNN()
        const embeddings = request.texts.map((text) =>
            Array.from(buildHashedTextEmbedding(text, dimensions)),
        )
        const response: EmbedBatchResponse = {
            id: request.id,
            ok: true,
            embeddings,
            backend: webnn.available ? "webnn-worker" : "hash-fallback",
            dimensions,
            probeMs: webnn.probeMs,
            reason: webnn.reason,
        }
        workerScope.postMessage(response)
    } catch (error: any) {
        const response: ErrorResponse = {
            id: request.id,
            ok: false,
            error: String(error?.message ?? error ?? "embedding worker failed"),
            backend: "hash-fallback",
            dimensions,
        }
        workerScope.postMessage(response)
    }
}

export {}
