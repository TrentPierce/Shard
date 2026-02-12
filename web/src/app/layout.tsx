import type { Metadata } from "next"
import "./globals.css"
import ErrorBoundary from "@/components/ErrorBoundary"
import { Providers } from "@/components/Providers"

export const metadata: Metadata = {
    title: "Shard — Distributed Inference Network",
    description:
        "Free, unlimited LLM access powered by a decentralized P2P inference mesh. Contribute compute, earn priority.",
    manifest: "/manifest.json",
    themeColor: "#06060e",
    viewport: "width=device-width, initial-scale=1, maximum-scale=5",
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
