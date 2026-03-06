import type { Metadata, Viewport } from "next"
import { IBM_Plex_Mono, Space_Grotesk } from "next/font/google"
import "./globals.css"
import { ErrorBoundary } from "@/components/ErrorBoundary"
import ServiceWorkerManager from "@/components/ServiceWorkerManager"
import { SiteNav } from "@/components/shell/SiteNav"
import { Providers } from "@/components/Providers"


const sans = Space_Grotesk({ subsets: ["latin"], variable: "--font-sans" })
const mono = IBM_Plex_Mono({ subsets: ["latin"], variable: "--font-mono", weight: ["400", "500"] })

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  colorScheme: "dark",
  themeColor: "#091540",
}

export const metadata: Metadata = {
  metadataBase: new URL(process.env.NEXT_PUBLIC_SITE_URL || "https://shardnetwork.live"),
  title: {
    default: "Shard | Live Telemetry Dashboard",
    template: "%s | Shard",
  },
  description: "Browser-powered distributed inference with live telemetry and OpenAI-compatible APIs.",
  icons: {
    icon: [{ url: "/brand-mark.png", type: "image/png" }],
    shortcut: [{ url: "/brand-mark.png", type: "image/png" }],
    apple: [{ url: "/brand-mark.png", type: "image/png" }],
  },
}

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${sans.variable} ${mono.variable}`}>
      <body>
        <a href="#main-content" className="skip-link">
          Skip to content
        </a>
        <SiteNav />
        <Providers>
          <ErrorBoundary>{children}</ErrorBoundary>
        </Providers>

        <ServiceWorkerManager />
      </body>
    </html>
  )
}
