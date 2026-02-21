"use client"

import { useEffect } from "react"

export default function ServiceWorkerManager() {
    useEffect(() => {
        if (!("serviceWorker" in navigator)) {
            return
        }

        const enableServiceWorker = process.env.NEXT_PUBLIC_ENABLE_SW === "true"

        if (enableServiceWorker) {
            navigator.serviceWorker
                .register("/sw.js", { updateViaCache: "none" })
                .catch((err) => {
                    console.error("[SW] Service Worker registration failed:", err)
                })
            return
        }

        navigator.serviceWorker
            .getRegistrations()
            .then((registrations) => {
                registrations.forEach((registration) => registration.unregister())
            })
            .catch(() => {
                // no-op: best-effort cleanup
            })
    }, [])

    return null
}
