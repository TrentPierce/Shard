import type { Metadata, Viewport } from "next"
import "./globals.css"
import ErrorBoundary from "@/components/ErrorBoundary"
import { Providers } from "@/components/Providers"

export const viewport: Viewport = {
    width: "device-width",
    initialScale: 1,
    maximumScale: 5,
    themeColor: "#06060e",
}

export const metadata: Metadata = {
    title: "Shard — Browser-Powered Distributed Inference",
    description:
        "Free, unlimited LLM access powered by a decentralized P2P inference mesh. Contribute browser compute via WebGPU, earn priority access. OpenAI-compatible API.",
    keywords: ["AI", "LLM", "distributed inference", "P2P", "WebGPU", "decentralized", "BitNet", "open source"],
    manifest: "/manifest.json",
    openGraph: {
        title: "Shard — Browser-Powered Distributed Inference",
        description:
            "Free, unlimited LLM access through a decentralized P2P mesh. Your browser becomes an AI compute node.",
        type: "website",
        siteName: "Shard Network",
        locale: "en_US",
    },
    twitter: {
        card: "summary_large_image",
        title: "Shard — Browser-Powered Distributed Inference",
        description:
            "Free LLM access through decentralized P2P compute. Contribute from your browser, earn priority.",
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
            <head>
                <link rel="preconnect" href="https://fonts.googleapis.com" />
                <link
                    rel="preconnect"
                    href="https://fonts.gstatic.com"
                    crossOrigin="anonymous"
                />
                <link
                    href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700;800&family=JetBrains+Mono:wght@400;500&display=swap"
                    rel="stylesheet"
                />
            </head>
            <body>
                <Providers>
                    <ErrorBoundary>{children}</ErrorBoundary>
                </Providers>
                {/* Service Worker Registration */}
                <script
                    dangerouslySetInnerHTML={{
                        __html: `
                            if ('serviceWorker' in navigator) {
                                navigator.serviceWorker.register('/sw.js').catch((err) => {
                                    console.error('[SW] Service Worker registration failed:', err);
                                });
                            }
                        `
                    }}
                />
            </body>
        </html>
    )
}
