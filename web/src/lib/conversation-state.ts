import { useCallback, useMemo, useState } from "react"
import type { ChatMessage } from "./api"
import {
    compactConversation,
    type PromptCompactionOptions,
    type PromptCompactionResult,
} from "./prompt-compaction"

export type ConversationSnapshot = {
    rawMessages: ChatMessage[]
    compactedMessages: ChatMessage[]
    compaction: PromptCompactionResult
}

export function useConversationState() {
    const [messages, setMessages] = useState<ChatMessage[]>([])

    const appendUserMessage = useCallback((content: string): ChatMessage => {
        const message: ChatMessage = {
            role: "user",
            content,
            timestamp: Date.now(),
        }
        setMessages((prev) => [...prev, message])
        return message
    }, [])

    const beginAssistantMessage = useCallback((): ChatMessage => {
        const message: ChatMessage = {
            role: "assistant",
            content: "",
            timestamp: Date.now(),
        }
        setMessages((prev) => [...prev, message])
        return message
    }, [])

    const appendAssistantToken = useCallback((token: string) => {
        setMessages((prev) => {
            const updated = [...prev]
            const last = updated[updated.length - 1]
            if (!last || last.role !== "assistant") {
                return updated
            }
            updated[updated.length - 1] = {
                ...last,
                content: last.content + token,
            }
            return updated
        })
    }, [])

    const replaceAssistantMessage = useCallback((content: string) => {
        setMessages((prev) => {
            const updated = [...prev]
            const last = updated[updated.length - 1]
            if (!last || last.role !== "assistant") {
                return updated
            }
            updated[updated.length - 1] = {
                ...last,
                content,
            }
            return updated
        })
    }, [])

    const buildHistory = useCallback(
        (nextUserMessage?: ChatMessage): ChatMessage[] =>
            nextUserMessage ? [...messages, nextUserMessage] : messages,
        [messages],
    )

    const snapshot = useCallback(
        (
            nextUserMessage?: ChatMessage,
            options?: PromptCompactionOptions,
        ): ConversationSnapshot => {
            const rawMessages = buildHistory(nextUserMessage)
            const compaction = compactConversation(rawMessages, options)
            return {
                rawMessages,
                compactedMessages: compaction.messages,
                compaction,
            }
        },
        [buildHistory],
    )

    const clearConversation = useCallback(() => {
        setMessages([])
    }, [])

    return useMemo(
        () => ({
            messages,
            appendUserMessage,
            beginAssistantMessage,
            appendAssistantToken,
            replaceAssistantMessage,
            buildHistory,
            snapshot,
            clearConversation,
        }),
        [
            messages,
            appendUserMessage,
            beginAssistantMessage,
            appendAssistantToken,
            replaceAssistantMessage,
            buildHistory,
            snapshot,
            clearConversation,
        ],
    )
}
