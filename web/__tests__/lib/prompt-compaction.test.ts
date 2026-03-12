import { compactConversation } from "@/lib/prompt-compaction"
import type { ChatMessage } from "@/lib/api"

function message(
  role: ChatMessage["role"],
  content: string,
  timestamp: number,
): ChatMessage {
  return { role, content, timestamp }
}

describe("prompt compaction", () => {
  it("returns the original conversation when under budget", () => {
    const input = [
      message("user", "Summarize this.", 1),
      message("assistant", "Here is a short summary.", 2),
    ]

    const result = compactConversation(input, {
      maxMessages: 4,
      maxTotalChars: 200,
    })

    expect(result.wasCompacted).toBe(false)
    expect(result.messages).toEqual(input)
    expect(result.compactedChars).toBe(result.originalChars)
  })

  it("summarizes older turns and keeps recent messages when over budget", () => {
    const input = Array.from({ length: 12 }, (_, index) =>
      message(
        index % 2 === 0 ? "user" : "assistant",
        `Turn ${index}: ${"x".repeat(180)}`,
        index + 1,
      ),
    )

    const result = compactConversation(input, {
      maxMessages: 6,
      maxTotalChars: 900,
      maxRecentMessages: 4,
      summaryMaxChars: 240,
      perMessageChars: 120,
    })

    expect(result.wasCompacted).toBe(true)
    expect(result.messages.length).toBeLessThanOrEqual(6)
    expect(result.compactedChars).toBeLessThanOrEqual(900)
    expect(result.messages[0]?.role).toBe("system")
    expect(result.messages[0]?.content).toContain("Browser conversation summary:")
    expect(result.messages.at(-1)?.content).toContain("Turn 11:")
  })

  it("keeps semantically relevant older messages alongside the summary", () => {
    const input = [
      message("user", "We should route simple prompts locally.", 1),
      message("assistant", "Yes, keep browser-first for low-complexity prompts.", 2),
      message("user", "Here is an unrelated gardening tangent.", 3),
      message("assistant", "Tomatoes need good soil drainage.", 4),
      message("user", "Now explain mesh forwarding for heavy prompts.", 5),
      message("assistant", "Heavy requests can route to healthier verifier peers.", 6),
    ]

    const result = compactConversation(input, {
      maxMessages: 4,
      maxTotalChars: 240,
      maxRecentMessages: 2,
      summaryMaxChars: 120,
      perMessageChars: 80,
      relevanceScores: [0.9, 0.8, 0.05, 0.02, 0.7, 0.6],
      semanticKeepCount: 1,
    })

    expect(result.wasCompacted).toBe(true)
    expect(result.semanticMessagesKept).toBe(1)
    expect(result.messages.some((entry) => entry.content.includes("route simple prompts locally"))).toBe(true)
  })
})
