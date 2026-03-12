import type { BackgroundAcceleration, WebNNWarmState } from "./scout-capability"

export type ContributionState =
  | "initializing"
  | "contributing"
  | "not_contributing"
  | "degraded"

export interface ContributionStatus {
  state: ContributionState
  reason: string
  capability?: "webgpu" | "wasm" | "unsupported"
  backgroundAcceleration?: BackgroundAcceleration
  lowPowerEligible?: boolean
  webnnProbeMs?: number
  webnnWarmState?: WebNNWarmState
  updated_at_ms: number
}

const STORAGE_KEY = "shard_contribution_status"
const EVENT_NAME = "shard:contribution-status"

function nowMs(): number {
  return Date.now()
}

export function readContributionStatus(): ContributionStatus | null {
  if (typeof window === "undefined") return null
  try {
    const raw = window.sessionStorage.getItem(STORAGE_KEY)
    if (!raw) return null
    return JSON.parse(raw) as ContributionStatus
  } catch {
    return null
  }
}

export function writeContributionStatus(status: ContributionStatus): void {
  if (typeof window === "undefined") return
  window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(status))
  window.dispatchEvent(
    new CustomEvent<ContributionStatus>(EVENT_NAME, {
      detail: status,
    })
  )
}

export function setContributionStatus(
  state: ContributionState,
  reason: string,
  capability?: "webgpu" | "wasm" | "unsupported",
  metadata?: {
    backgroundAcceleration?: BackgroundAcceleration
    lowPowerEligible?: boolean
    webnnProbeMs?: number
    webnnWarmState?: WebNNWarmState
  }
): ContributionStatus {
  const status: ContributionStatus = {
    state,
    reason,
    capability,
    backgroundAcceleration: metadata?.backgroundAcceleration,
    lowPowerEligible: metadata?.lowPowerEligible,
    webnnProbeMs: metadata?.webnnProbeMs,
    webnnWarmState: metadata?.webnnWarmState,
    updated_at_ms: nowMs(),
  }
  writeContributionStatus(status)
  return status
}

export const CONTRIBUTION_STATUS_EVENT = EVENT_NAME
