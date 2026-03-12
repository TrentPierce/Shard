import "@testing-library/jest-dom"

jest.mock("@/lib/scout-engine", () => ({
  getActiveEngine: jest.fn(),
  generateDrafts: jest.fn(),
}))

jest.mock("@/lib/scout-draft", () => ({
  getScoutId: jest.fn(),
  pollForWork: jest.fn(),
  reportScoutClientEvent: jest.fn(),
  submitDraft: jest.fn(),
}))

describe("swarm handleScoutWork", () => {
  beforeEach(() => {
    jest.resetModules()
    jest.clearAllMocks()
  })

  it("skips submission when engine is unavailable and no real draft can be produced", async () => {
    const { getActiveEngine } = await import("@/lib/scout-engine")
    const { getScoutId, submitDraft, reportScoutClientEvent } = await import("@/lib/scout-draft")

    ;(getActiveEngine as jest.Mock).mockReturnValue(null)
    ;(getScoutId as jest.Mock).mockReturnValue("scout-test")
    const { handleScoutWork } = await import("@/lib/swarm")

    const result = await handleScoutWork({
      request_id: "work-123",
      prompt_context: "Hello world",
      min_tokens: 4,
    })

    expect(result.success).toBe(false)
    expect(result.detail).toContain("No usable draft produced")
    expect(submitDraft).not.toHaveBeenCalled()
    expect(reportScoutClientEvent).toHaveBeenCalledWith(
      "fallback_draft_used",
      "engine_uninitialized",
      undefined,
      undefined,
      { apiBase: undefined },
    )
  })
})
