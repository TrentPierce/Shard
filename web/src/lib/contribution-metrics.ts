export interface ContributionMetrics {
  draftTokens: number
  verifiedTokens: number
}

const DRAFT_KEY = "shard_scout_contributed_tokens"
const VERIFIED_KEY = "shard_verified_tokens"

function readNumber(key: string): number {
  if (typeof window === "undefined") return 0
  const value = window.sessionStorage.getItem(key)
  if (!value) return 0
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : 0
}

function writeNumber(key: string, value: number) {
  if (typeof window === "undefined") return
  window.sessionStorage.setItem(key, String(Math.max(0, Math.floor(value))))
}

export function readContributionMetrics(): ContributionMetrics {
  return {
    draftTokens: readNumber(DRAFT_KEY),
    verifiedTokens: readNumber(VERIFIED_KEY),
  }
}

export function resetSessionMetrics(): ContributionMetrics {
  writeNumber(DRAFT_KEY, 0)
  writeNumber(VERIFIED_KEY, 0)
  return readContributionMetrics()
}

export function incrementDraftTokens(by = 1): ContributionMetrics {
  const current = readContributionMetrics()
  writeNumber(DRAFT_KEY, current.draftTokens + by)
  return readContributionMetrics()
}

export function incrementVerifiedTokens(by = 1): ContributionMetrics {
  const current = readContributionMetrics()
  writeNumber(VERIFIED_KEY, current.verifiedTokens + by)
  return readContributionMetrics()
}
