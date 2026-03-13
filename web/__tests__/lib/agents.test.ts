jest.mock("@/lib/runtime", () => ({
  canUseLocalDaemonFallback: () => false,
  getPreferredLocalDaemonBase: async () => "http://127.0.0.1:9091",
  localDaemonUrls: (path: string) => [`http://127.0.0.1:9091${path}`],
}))

describe("agent provenance api client", () => {
  beforeEach(() => {
    jest.resetModules()
    global.fetch = jest.fn()
  })

  it("submits a research brief workflow with merged policy defaults", async () => {
    const { submitResearchBriefTask } = await import("@/lib/agents")
    ;(global.fetch as jest.Mock).mockResolvedValue({
      ok: true,
      status: 201,
      statusText: "Created",
      json: async () => ({
        ok: true,
        execution: {
          execution_id: "exec-1",
          workflow_kind: "research_brief",
          status: "completed",
          created_at_ms: 1,
          updated_at_ms: 2,
          source_count: 1,
        },
        provenance: {
          execution_id: "exec-1",
          root_receipt_id: "rcpt-1",
          nodes: [],
          edges: [],
          incomplete: false,
        },
        receipts: [],
      }),
    })

    const response = await submitResearchBriefTask({
      question: "What changed?",
      sources: [{ id: "s1", content: "A market shifted." }],
      policy: { trust_tier: "verified_mesh", allowed_supply_tiers: ["private"] },
    })

    expect(response.execution.execution_id).toBe("exec-1")
    const [, init] = (global.fetch as jest.Mock).mock.calls[0]
    const body = JSON.parse(init.body)
    expect(body.workflow_kind).toBe("research_brief")
    expect(body.policy.trust_tier).toBe("verified_mesh")
    expect(body.policy.allowed_supply_tiers).toEqual(["private"])
    expect(body.policy.fallback_order).toEqual(["personal", "private", "public"])
  })

  it("fetches provenance from the execution endpoint", async () => {
    const { fetchExecutionProvenance } = await import("@/lib/agents")
    ;(global.fetch as jest.Mock).mockResolvedValue({
      ok: true,
      status: 200,
      statusText: "OK",
      json: async () => ({
        ok: true,
        provenance: {
          execution_id: "exec-2",
          root_receipt_id: "rcpt-root",
          nodes: [{ receipt_id: "rcpt-root", step_id: "workflow", attempt_id: "a1", event_kind: "planned", timestamp_ms: 1 }],
          edges: [],
          incomplete: true,
        },
      }),
    })

    const response = await fetchExecutionProvenance("exec-2")

    expect(response.provenance.execution_id).toBe("exec-2")
    expect((global.fetch as jest.Mock).mock.calls[0][0]).toContain("/v1/executions/exec-2/provenance")
  })
})
