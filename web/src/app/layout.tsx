import type { Metadata, Viewport } from "next"
import "./globals.css"
import ErrorBoundary from "@/components/ErrorBoundary"
import { Providers } from "@/components/Providers"
import ServiceWorkerManager from "@/components/ServiceWorkerManager"

export const viewport: Viewport = {
    width: "device-width",
    initialScale: 1,
    maximumScale: 5,
    colorScheme: "dark",
    themeColor: "#030307",
}

export const metadata: Metadata = {
    metadataBase: new URL(process.env.NEXT_PUBLIC_SITE_URL || "https://shardnetwork.live"),
    title: {
        default: "Shard — Browser-Powered Distributed Inference",
        template: "%s | Shard Network",
    },
    description:
        "Free, unlimited LLM access through a decentralized P2P mesh. Contribute compute from your browser via WebGPU, earn priority access. OpenAI-compatible API.",
    keywords: ["AI", "LLM", "distributed inference", "P2P", "WebGPU", "decentralized", "BitNet", "speculative decoding", "open source", "free AI"],
    authors: [{ name: "Shard Network" }],
    creator: "Shard Network",
    publisher: "Shard Network",
    formatDetection: {
        email: false,
        address: false,
        telephone: false,
    },
    alternates: {
        canonical: "/",
        languages: {
            en: "/",
        },
    },
    robots: {
        index: true,
        follow: true,
        googleBot: {
            index: true,
            follow: true,
            "max-video-preview": -1,
            "max-image-preview": "large",
            "max-snippet": -1,
        },
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
            "Free LLM access through decentralized P2P compute. Contribute from your browser, earn priority access.",
        images: ["/og-image.svg"],
        creator: "@shardnetwork",
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
                <a href="#main-content" className="skip-link">
                    Skip to content
                </a>
                <Providers>
                    <ErrorBoundary>{children}</ErrorBoundary>
                </Providers>
                <ServiceWorkerManager />
            </body>
        </html>
    )
}
