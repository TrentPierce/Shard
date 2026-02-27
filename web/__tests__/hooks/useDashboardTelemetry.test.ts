import { renderHook } from "@testing-library/react"
import { useDashboardTelemetry } from "@/hooks/useDashboardTelemetry"
import { useSwarmTelemetry } from "@/hooks/useSwarmTelemetry"

jest.mock("@/hooks/useSwarmTelemetry", () => ({
  useSwarmTelemetry: jest.fn(),
}))

describe("useDashboardTelemetry", () => {
  const useSwarmTelemetryMock = useSwarmTelemetry as jest.Mock

  it("propagates ready status and live connectivity", () => {
    useSwarmTelemetryMock.mockReturnValue({
      telemetry: {
        healthState: "ready",
        globalTflops: 4,
        scoutCount: 2,
        shardCount: 3,
        throughputHistory: [{ timestamp: "10:00:00", tflops: 4 }],
        contributors: [],
        totalTokensGenerated: 120,
      },
      isConnected: true,
      isLoading: false,
      statusLabel: "READY",
    })

    const { result } = renderHook(() => useDashboardTelemetry())
    expect(result.current.healthState).toBe("ready")
    expect(result.current.statusLabel).toBe("READY")
    expect(result.current.isLive).toBe(true)
  })

  it("propagates degraded and unavailable transitions", () => {
    useSwarmTelemetryMock.mockReturnValueOnce({
      telemetry: {
        healthState: "degraded",
        globalTflops: 0,
        scoutCount: 0,
        shardCount: 1,
        throughputHistory: [],
        contributors: [],
        totalTokensGenerated: 0,
      },
      isConnected: true,
      isLoading: false,
      statusLabel: "DEGRADED",
    })

    const degraded = renderHook(() => useDashboardTelemetry())
    expect(degraded.result.current.healthState).toBe("degraded")
    expect(degraded.result.current.statusLabel).toBe("DEGRADED")

    useSwarmTelemetryMock.mockReturnValueOnce({
      telemetry: {
        healthState: "unavailable",
        globalTflops: 0,
        scoutCount: 0,
        shardCount: 0,
        throughputHistory: [],
        contributors: [],
        totalTokensGenerated: 0,
      },
      isConnected: false,
      isLoading: false,
      statusLabel: "OFFLINE",
    })

    const unavailable = renderHook(() => useDashboardTelemetry())
    expect(unavailable.result.current.healthState).toBe("unavailable")
    expect(unavailable.result.current.statusLabel).toBe("OFFLINE")
    expect(unavailable.result.current.isLive).toBe(false)
  })
})
