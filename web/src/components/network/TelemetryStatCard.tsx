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
    <div className={`network-card network-card--${accent}`}>
      <p className="network-card__label">{label}</p>
      <p className="network-card__value">{value}</p>
      <p className="network-card__hint">{hint}</p>
    </div>
  )
}
