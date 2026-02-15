export type Contributor = {
  id: string
  role: "Scout" | "Shard"
  tokensProcessed: number
  efficiency: number
}

export type ThroughputSample = {
  timestamp: string
  tflops: number
}

export type SwarmTelemetrySnapshot = {
  globalTflops: number
  scoutCount: number
  shardCount: number
  throughputHistory: ThroughputSample[]
  contributors: Contributor[]
}

const baseContributors: Contributor[] = [
  { id: "scout-alpha-03", role: "Scout", tokensProcessed: 1_820_420, efficiency: 88 },
  { id: "desktop-eu-west-17", role: "Shard", tokensProcessed: 1_501_993, efficiency: 93 },
  { id: "scout-singapore-02", role: "Scout", tokensProcessed: 1_146_812, efficiency: 86 },
  { id: "desktop-na-central-08", role: "Shard", tokensProcessed: 990_041, efficiency: 90 },
  { id: "scout-latam-11", role: "Scout", tokensProcessed: 876_654, efficiency: 84 },
]

const buildHistory = (): ThroughputSample[] => {
  const now = Date.now()

  return Array.from({ length: 18 }, (_, index) => {
    const minutesAgo = 17 - index
    const time = new Date(now - minutesAgo * 60_000)
    const jitter = Math.sin(index / 2.2) * 7 + (index % 4) * 1.8

    return {
      timestamp: time.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
      tflops: Number((102 + jitter).toFixed(2)),
    }
  })
}

export const createInitialTelemetry = (): SwarmTelemetrySnapshot => {
  const throughputHistory = buildHistory()

  return {
    globalTflops: throughputHistory[throughputHistory.length - 1].tflops,
    scoutCount: 138,
    shardCount: 64,
    throughputHistory,
    contributors: baseContributors,
  }
}

export const tickTelemetry = (
  current: SwarmTelemetrySnapshot,
): SwarmTelemetrySnapshot => {
  const now = new Date()
  const trend = Math.sin(now.getSeconds() / 8) * 3.5
  const variance = (Math.random() - 0.5) * 4
  const nextTflops = Math.max(65, Number((current.globalTflops + trend + variance).toFixed(2)))

  const nextHistory = [
    ...current.throughputHistory.slice(-17),
    {
      timestamp: now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
      tflops: nextTflops,
    },
  ]

  const nextContributors = current.contributors
    .map((entry) => ({
      ...entry,
      tokensProcessed: entry.tokensProcessed + Math.floor(1300 + Math.random() * 5000),
      efficiency: Math.min(99, Math.max(70, entry.efficiency + Math.floor(Math.random() * 5) - 2)),
    }))
    .sort((a, b) => b.tokensProcessed - a.tokensProcessed)

  return {
    globalTflops: nextTflops,
    scoutCount: Math.max(40, current.scoutCount + Math.floor(Math.random() * 5) - 2),
    shardCount: Math.max(15, current.shardCount + Math.floor(Math.random() * 5) - 2),
    throughputHistory: nextHistory,
    contributors: nextContributors,
  }
}
