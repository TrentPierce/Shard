type TelemetryStatCardProps = {
  label: string
  value: string
  hint: string
  accent?: "cyan" | "violet" | "emerald"
}

export default function TelemetryStatCard({
  label,
  value,
  hint,
  accent = "cyan",
}: TelemetryStatCardProps) {
  return (
    <div className="card text-center" style={{ borderTop: `2px solid var(--accent-${accent})` }}>
      <p className="text-secondary text-mono mb-sm">{label}</p>
      <h2 className="mb-sm text-primary">{value}</h2>
      <p className="text-muted text-sm">{hint}</p>
    </div>
  )
}
