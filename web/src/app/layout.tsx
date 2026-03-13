import type { Metadata, Viewport } from "next"
import { IBM_Plex_Mono, Space_Grotesk } from "next/font/google"
import "./globals.css"
import { ErrorBoundary } from "@/components/ErrorBoundary"
import ServiceWorkerManager from "@/components/ServiceWorkerManager"
import Footer from "@/components/Footer"
import { SiteNav } from "@/components/shell/SiteNav"
import { Providers } from "@/components/Providers"


const sans = Space_Grotesk({ subsets: ["latin"], variable: "--font-sans" })
const mono = IBM_Plex_Mono({ subsets: ["latin"], variable: "--font-mono", weight: ["400", "500"] })

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  colorScheme: "dark",
  themeColor: "#07131d",
}

export const metadata: Metadata = {
  metadataBase: new URL(process.env.NEXT_PUBLIC_SITE_URL || "https://shardnetwork.live"),
  title: {
    default: "Shard | See Why Every Agent Step Ran There",
    template: "%s | Shard",
  },
  description:
    "Shard helps AI teams see why each workflow step used personal, private, or public capacity with receipts, provenance graphs, and policy-aware routing.",
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
        <Footer />
      </body>
    </html>
  )
}
