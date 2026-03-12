import { buildHashedTextEmbedding, cosineSimilarity } from "@/lib/embedding-core"

describe("embedding core", () => {
  it("keeps semantically similar prompts closer than unrelated prompts", () => {
    const routeA = buildHashedTextEmbedding(
      "Route a simple prompt locally in the browser before using the network.",
    )
    const routeB = buildHashedTextEmbedding(
      "The browser should answer simple prompts locally before escalating to the network.",
    )
    const unrelated = buildHashedTextEmbedding(
      "Bake sourdough bread with steam and a preheated dutch oven.",
    )

    expect(cosineSimilarity(routeA, routeB)).toBeGreaterThan(0.35)
    expect(cosineSimilarity(routeA, unrelated)).toBeLessThan(
      cosineSimilarity(routeA, routeB),
    )
  })
})
