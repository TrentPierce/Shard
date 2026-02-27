export type HealthState = "ready" | "degraded" | "unavailable"

type HealthLikePayload = {
  status?: unknown
  ready_for_inference?: unknown
}

function normalizeStatus(value: unknown): string {
  return typeof value === "string" ? value.trim().toLowerCase() : ""
}

export function deriveHealthState(
  payload: HealthLikePayload | null | undefined,
  backendReachable: boolean,
): HealthState {
  if (!backendReachable) return "unavailable"

  const status = normalizeStatus(payload?.status)
  const readyForInference = payload?.ready_for_inference

  if (status === "unavailable") return "unavailable"
  if (readyForInference === true) return "ready"

  // Backward-compatible: many existing callers only emit `status: "ok"`.
  if (status === "ok" && readyForInference !== false) return "ready"

  return "degraded"
}

export function healthStateToLegacyStatus(state: HealthState): "ok" | "degraded" | "unavailable" {
  switch (state) {
    case "ready":
      return "ok"
    case "degraded":
      return "degraded"
    case "unavailable":
      return "unavailable"
  }
}
