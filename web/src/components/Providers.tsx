"use client"

import { QueryClient, QueryClientProvider } from "@tanstack/react-query"

/**
 * React Query client provider for Shard web app.
 *
 * Provides server state management with:
 * - Automatic caching
 * - Background refetching
 * - Optimistic updates
 * - Retry logic
 */
export function Providers({ children }: { children: React.ReactNode }) {
    const [queryClient] = useState(() => new QueryClient({
        defaultOptions: {
            queries: {
                staleTime: 10 * 1000, // 10 seconds
                refetchOnWindowFocus: false,
                retry: 3,
                retryDelay: (attemptIndex) => Math.min(1000 * 2 ** attemptIndex, 30000),
            },
        },
    }))

    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
}

import { useState } from "react"
