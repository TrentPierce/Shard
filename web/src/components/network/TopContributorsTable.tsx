import type { Contributor } from "@/lib/mockSwarmTelemetry"

type TopContributorsTableProps = {
  contributors: Contributor[]
}

const formatTokens = (tokens: number) =>
  new Intl.NumberFormat("en-US", {
    notation: "compact",
    maximumFractionDigits: 2,
  }).format(tokens)

export default function TopContributorsTable({ contributors }: TopContributorsTableProps) {
  return (
    <div className="card" style={{ gridColumn: '1 / -1' }}>
      <div className="mb-md">
        <h2 className="text-mono">Top Contributors</h2>
        <span className="text-secondary">live contribution ranking</span>
      </div>
      <div className="leaderboard">
        {contributors.map((contributor, index) => (
          <div className="leaderboard__row" key={contributor.id}>
            <span className="leaderboard__rank">#{index + 1}</span>
            <div className="leaderboard__identity">
              <strong>{contributor.id}</strong>
              <small className={`leaderboard__role leaderboard__role--${contributor.role.toLowerCase()}`}>{contributor.role}</small>
            </div>
            <div className="leaderboard__metrics">
              <strong>{formatTokens(contributor.tokensProcessed)}</strong>
              <small>{contributor.efficiency}% verification efficiency</small>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
