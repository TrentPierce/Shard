import {
  deriveHealthState,
  healthStateToLegacyStatus,
} from "@/lib/server/health-state"

describe("health-state mapping", () => {
  it("maps ready payloads to ready/ok", () => {
    expect(
      deriveHealthState({ status: "ok", ready_for_inference: true }, true),
    ).toBe("ready")
    expect(healthStateToLegacyStatus("ready")).toBe("ok")
  })

  it("maps backend reachability failures to unavailable", () => {
    expect(deriveHealthState({ status: "ok" }, false)).toBe("unavailable")
    expect(healthStateToLegacyStatus("unavailable")).toBe("unavailable")
  })

  it("maps non-ready but reachable payloads to degraded", () => {
    expect(
      deriveHealthState({ status: "ok", ready_for_inference: false }, true),
    ).toBe("degraded")
    expect(deriveHealthState({ status: "degraded" }, true)).toBe("degraded")
  })
})
