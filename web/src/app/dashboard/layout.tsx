import type { Metadata } from "next"

export const metadata: Metadata = {
  title: "Shard Dashboard — Network Analytics",
  description:
    "Monitor TPS, topology health, alerts, and reliability metrics for Shard distributed inference workloads.",
  alternates: { canonical: "/dashboard" },
  openGraph: {
    title: "Shard Dashboard — Network Analytics",
    description:
      "Operational analytics for Shard inference: throughput, health, and reliability at a glance.",
    url: "/dashboard",
    images: ["/og-image.svg"],
  },
  twitter: {
    card: "summary_large_image",
    title: "Shard Dashboard — Network Analytics",
    description: "Operational analytics and reliability insights for the Shard mesh.",
    images: ["/og-image.svg"],
  },
}

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  return children
}
