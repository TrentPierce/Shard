import type { Metadata } from "next"

export const metadata: Metadata = {
  title: "Shard Network — Live Mesh Overview",
  description:
    "Inspect live node health, topology, and mesh throughput across the distributed Shard inference network.",
  alternates: { canonical: "/network" },
  openGraph: {
    title: "Shard Network — Live Mesh Overview",
    description:
      "Inspect live node health, topology, and throughput across Shard's distributed inference mesh.",
    url: "/network",
    images: ["/og-image.svg"],
  },
  twitter: {
    card: "summary_large_image",
    title: "Shard Network — Live Mesh Overview",
    description: "Real-time node and topology telemetry for the Shard mesh.",
    images: ["/og-image.svg"],
  },
}

export default function NetworkLayout({ children }: { children: React.ReactNode }) {
  return children
}
