import { checkWebGPUSupport, generateBrowserChatCompletion } from "./webllm"
import type { ChatMessage } from "./api"

let browserRuntimeSupportPromise: Promise<boolean> | null = null

export async function canUseBrowserChatRuntime(): Promise<boolean> {
    if (!browserRuntimeSupportPromise) {
        browserRuntimeSupportPromise = checkWebGPUSupport()
            .then((status) => status.supported)
            .catch(() => false)
    }
    return browserRuntimeSupportPromise
}

export async function sendBrowserChatMessage(
    history: ChatMessage[],
    onToken: (token: string) => void,
    onDone: () => void,
): Promise<void> {
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

    onDone()
}
