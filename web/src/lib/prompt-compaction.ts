import type { ChatMessage } from "./api"

export type PromptCompactionOptions = {
    maxMessages?: number
    maxTotalChars?: number
    maxRecentMessages?: number
    summaryMaxChars?: number
    perMessageChars?: number
}

export type PromptCompactionResult = {
    messages: ChatMessage[]
    wasCompacted: boolean
    originalMessageCount: number
    compactedMessageCount: number
    originalChars: number
    compactedChars: number
    summaryChars: number
}

const DEFAULT_OPTIONS: Required<PromptCompactionOptions> = {
    maxMessages: 10,
    maxTotalChars: 4500,
    maxRecentMessages: 8,
    summaryMaxChars: 1000,
    perMessageChars: 320,
}

function countChars(messages: ChatMessage[]): number {
    return messages.reduce((total, message) => total + message.content.length, 0)
}

function compressText(text: string, maxChars: number): string {
    const trimmed = text.replace(/\s+/g, " ").trim()
    if (trimmed.length <= maxChars) {
        return trimmed
    }
    if (maxChars <= 24) {
        return trimmed.slice(0, maxChars)
    }
    const headChars = Math.max(12, Math.floor(maxChars * 0.7))
    const tailChars = Math.max(8, maxChars - headChars - 1)
    return `${trimmed.slice(0, headChars)}…${trimmed.slice(-tailChars)}`
}

function buildSummaryMessage(
    olderMessages: ChatMessage[],
    options: Required<PromptCompactionOptions>,
): ChatMessage | null {
    if (olderMessages.length === 0) {
        return null
    }

    const lines: string[] = []
    let usedChars = 0
    for (const message of olderMessages) {
        const roleLabel =
            message.role === "user"
                ? "User"
                : message.role === "assistant"
                    ? "Assistant"
                    : "System"
        const snippet = compressText(message.content, options.perMessageChars)
        const nextLine = `- ${roleLabel}: ${snippet}`
        if (usedChars + nextLine.length > options.summaryMaxChars) {
            break
        }
        lines.push(nextLine)
        usedChars += nextLine.length
    }

    if (lines.length === 0) {
        return null
    }

    return {
        role: "system",
        content: `Browser conversation summary:\n${lines.join("\n")}`,
        timestamp: olderMessages[olderMessages.length - 1]?.timestamp ?? Date.now(),
    }
}

function trimMessagesToBudget(
    messages: ChatMessage[],
    maxTotalChars: number,
    perMessageChars: number,
): ChatMessage[] {
    let current = [...messages]
    while (countChars(current) > maxTotalChars && current.length > 0) {
        const summaryIdx = current.findIndex((message) => message.role === "system")
        const targetIdx = summaryIdx >= 0 ? summaryIdx : 0
        const target = current[targetIdx]
        const nextLimit = Math.max(96, Math.floor(target.content.length * 0.75))
        const trimmedTarget = {
            ...target,
            content: compressText(target.content, Math.min(nextLimit, perMessageChars)),
        }
        if (trimmedTarget.content === target.content) {
            if (current.length <= 1) {
                break
            }
            current.splice(targetIdx, 1)
        } else {
            current[targetIdx] = trimmedTarget
        }
    }
    return current
}

export function compactConversation(
    messages: ChatMessage[],
    options: PromptCompactionOptions = {},
): PromptCompactionResult {
    const resolved = { ...DEFAULT_OPTIONS, ...options }
    const originalChars = countChars(messages)
    const originalMessageCount = messages.length

    if (
        originalMessageCount <= resolved.maxMessages &&
        originalChars <= resolved.maxTotalChars
    ) {
        return {
            messages,
            wasCompacted: false,
            originalMessageCount,
            compactedMessageCount: originalMessageCount,
            originalChars,
            compactedChars: originalChars,
            summaryChars: 0,
        }
    }

    const recentCount = Math.min(resolved.maxRecentMessages, messages.length)
    const recentMessages = messages.slice(-recentCount)
    const olderMessages = messages.slice(0, Math.max(0, messages.length - recentCount))
    const summaryMessage = buildSummaryMessage(olderMessages, resolved)
    let compactedMessages = summaryMessage
        ? [summaryMessage, ...recentMessages]
        : recentMessages

    compactedMessages = compactedMessages.map((message) => ({
        ...message,
        content: compressText(
            message.content,
            message.role === "system"
                ? resolved.summaryMaxChars
                : Math.max(resolved.perMessageChars, 192),
        ),
    }))

    if (
        compactedMessages.length > resolved.maxMessages ||
        countChars(compactedMessages) > resolved.maxTotalChars
    ) {
        const keepRecent = Math.max(2, resolved.maxMessages - (summaryMessage ? 1 : 0))
        compactedMessages = [
            ...(summaryMessage ? [compactedMessages[0]] : []),
            ...compactedMessages.slice(-(keepRecent)),
        ]
        compactedMessages = trimMessagesToBudget(
            compactedMessages,
            resolved.maxTotalChars,
            resolved.perMessageChars,
        )
    }

    return {
        messages: compactedMessages,
        wasCompacted: true,
        originalMessageCount,
        compactedMessageCount: compactedMessages.length,
        originalChars,
        compactedChars: countChars(compactedMessages),
        summaryChars: summaryMessage?.content.length ?? 0,
    }
}
