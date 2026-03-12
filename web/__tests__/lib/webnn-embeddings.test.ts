import { rankMessagesBySemanticRelevance } from "@/lib/webnn-embeddings"
import type { ChatMessage } from "@/lib/api"

function message(role: ChatMessage["role"], content: string, timestamp: number): ChatMessage {
  return { role, content, timestamp }
}

describe("webnn embeddings relevance", () => {
  it("scores matching routing history above unrelated history", async () => {
    const messages = [
      message("user", "How does Shard route simple prompts locally?", 1),
      message("assistant", "It uses a browser-first router and escalates hard work.", 2),
      message("user", "What is a good pasta dough hydration ratio?", 3),
    ]

    const result = await rankMessagesBySemanticRelevance(
      messages,
      "Explain how the browser router escalates hard prompts to the network.",
    )

    expect(result.scores).toHaveLength(3)
    expect(result.scores[0]).toBeGreaterThan(result.scores[2])
    expect(result.scores[1]).toBeGreaterThan(result.scores[2])
  })
})
