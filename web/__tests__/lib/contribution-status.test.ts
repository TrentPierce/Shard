import {
  CONTRIBUTION_STATUS_EVENT,
  readContributionStatus,
  setContributionStatus,
} from "@/lib/contribution-status"

describe("contribution-status telemetry", () => {
  beforeEach(() => {
    window.sessionStorage.clear()
  })

  it("stores status and emits contribution event", () => {
    const handler = jest.fn()
    window.addEventListener(CONTRIBUTION_STATUS_EVENT, handler as EventListener)

    const status = setContributionStatus(
      "not_contributing",
      "WebGPU not available",
      "unsupported"
    )

    const stored = readContributionStatus()
    expect(stored?.state).toBe("not_contributing")
    expect(stored?.reason).toContain("WebGPU not available")
    expect(status.capability).toBe("unsupported")
    expect(handler).toHaveBeenCalledTimes(1)

    window.removeEventListener(CONTRIBUTION_STATUS_EVENT, handler as EventListener)
  })
})
