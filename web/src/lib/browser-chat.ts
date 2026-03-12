import { checkWebGPUSupport, generateBrowserChatCompletion, isModelCached } from "./webllm"
import { emitChatSuccess, type ChatExecutionResult, type ChatMessage } from "./api"

let browserRuntimeSupportPromise: Promise<boolean> | null = null
let browserRuntimePreferencePromise: Promise<boolean> | null = null

function isFirefoxBrowser(): boolean {
    if (typeof navigator === "undefined") return false
    const ua = navigator.userAgent.toLowerCase()
    return ua.includes("firefox") && !ua.includes("seamonkey")
}

export async function canUseBrowserChatRuntime(): Promise<boolean> {
    if (!browserRuntimeSupportPromise) {
        browserRuntimeSupportPromise = checkWebGPUSupport()
            .then((status) => status.supported)
            .catch(() => false)
    }
    return browserRuntimeSupportPromise
}

export async function shouldPreferBrowserChatRuntime(): Promise<boolean> {
    if (!browserRuntimePreferencePromise) {
        browserRuntimePreferencePromise = (async () => {
            const supported = await canUseBrowserChatRuntime()
            if (!supported) {
                return false
            }
            if (isFirefoxBrowser()) {
                return false
            }
            try {
                return await isModelCached()
            } catch {
                return false
            }
        })()
    }
    return browserRuntimePreferencePromise
}

export async function sendBrowserChatMessage(
    history: ChatMessage[],
    onToken: (token: string) => void,
    onDone: () => void,
): Promise<ChatExecutionResult> {
    const startedAt = performance.now()
    const response = await generateBrowserChatCompletion(
        history.map((message) => ({
            role: message.role,
            content: message.content,
        })),
        {
            maxTokens: 192,
            temperature: 0.2,
            onToken,
        },
    )

    if (!response.success) {
        throw new Error(response.error || "Local browser response failed")
    }

    emitChatSuccess({
        latencyMs: Math.round(performance.now() - startedAt),
        inferenceMode: "browser_local",
        transport: "browser_local",
    })
    onDone()
    return {
        latencyMs: Math.round(performance.now() - startedAt),
        inferenceMode: "browser_local",
        transport: "browser_local",
    }
}
