const WORK_AFFINITY_TTL_MS = 2 * 60 * 1000

type AffinityEntry = {
  backendBase: string
  expiresAtMs: number
}

const workAffinity = new Map<string, AffinityEntry>()

function normalizeBackendBase(value: string): string {
  const raw = String(value || "").trim()
  if (!raw) return ""
  try {
    const parsed = new URL(raw)
    return `${parsed.protocol}//${parsed.host}`
  } catch {
    return raw.replace(/\/+$/, "")
  }
}

function prune(nowMs: number): void {
  for (const [workId, entry] of workAffinity.entries()) {
    if (entry.expiresAtMs <= nowMs) {
      workAffinity.delete(workId)
    }
  }
}

export function rememberWorkAffinity(workId: string, backendUrl: string): void {
  const trimmedWorkId = String(workId || "").trim()
  const backendBase = normalizeBackendBase(backendUrl)
  if (!trimmedWorkId || !backendBase) return
  const nowMs = Date.now()
  prune(nowMs)
  workAffinity.set(trimmedWorkId, {
    backendBase,
    expiresAtMs: nowMs + WORK_AFFINITY_TTL_MS,
  })
}

export function preferredCandidateForWork(workId: string, path: string): string | null {
  const trimmedWorkId = String(workId || "").trim()
  if (!trimmedWorkId) return null
  const nowMs = Date.now()
  prune(nowMs)
  const entry = workAffinity.get(trimmedWorkId)
  if (!entry) return null
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  return `${entry.backendBase}${cleanPath}`
}

export function clearWorkAffinity(workId: string): void {
  const trimmedWorkId = String(workId || "").trim()
  if (!trimmedWorkId) return
  workAffinity.delete(trimmedWorkId)
}
