import type { Metadata, Viewport } from "next"
import "./globals.css"
import ErrorBoundary from "@/components/ErrorBoundary"
import { Providers } from "@/components/Providers"
import ServiceWorkerManager from "@/components/ServiceWorkerManager"

export const viewport: Viewport = {
    width: "device-width",
    initialScale: 1,
    maximumScale: 5,
    colorScheme: "dark light",
    themeColor: [
        { media: "(prefers-color-scheme: dark)", color: "#06060e" },
        { media: "(prefers-color-scheme: light)", color: "#f8fafc" },
    ],
}

export const metadata: Metadata = {
    metadataBase: new URL(process.env.NEXT_PUBLIC_SITE_URL || "https://shardnetwork.live"),
    title: "Shard — Browser-Powered Distributed Inference",
    description:
        "Free, unlimited LLM access powered by a decentralized P2P inference mesh. Contribute browser compute via WebGPU, earn priority access. OpenAI-compatible API.",
    keywords: ["AI", "LLM", "distributed inference", "P2P", "WebGPU", "decentralized", "BitNet", "open source"],
    alternates: {
        canonical: "/",
    },
    robots: {
        index: true,
        follow: true,
    },
    manifest: "/manifest.json",
    openGraph: {
        title: "Shard — Browser-Powered Distributed Inference",
        description:
            "Free, unlimited LLM access through a decentralized P2P mesh. Your browser becomes an AI compute node.",
        type: "website",
        siteName: "Shard Network",
        locale: "en_US",
        url: "/",
        images: [
            {
                url: "/og-image.svg",
                width: 1200,
                height: 630,
                alt: "Shard distributed inference mesh",
            },
        ],
    },
    twitter: {
        card: "summary_large_image",
        title: "Shard — Browser-Powered Distributed Inference",
        description:
            "Free LLM access through decentralized P2P compute. Contribute from your browser, earn priority.",
        images: ["/og-image.svg"],
    },
    appleWebApp: {
        capable: true,
        statusBarStyle: "black-translucent",
        title: "Shard",
    },
    icons: [
        {
            url: "/icon-192.png",
            sizes: "192x192",
            type: "image/png",
        },
        {
            url: "/icon-512.png",
            sizes: "512x512",
            type: "image/png",
        },
    ],
}

export default function RootLayout({
    children,
}: {
    children: React.ReactNode
}) {
    return (
        <html lang="en">
            <body>
                <Providers>
                    <ErrorBoundary>{children}</ErrorBoundary>
                </Providers>
                <ServiceWorkerManager />
            </body>
        </html>
    )
}
