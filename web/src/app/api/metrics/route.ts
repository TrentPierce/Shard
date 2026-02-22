import { NextResponse } from "next/server"
import { shardBackendUrl } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET() {
    const url = shardBackendUrl("/metrics")

    try {
        const response = await fetch(url, {
            signal: AbortSignal.timeout(8000),
            cache: "no-store",
        })
        const text = await response.text()

        return new NextResponse(text, {
            status: response.status,
            headers: {
                "Content-Type": "text/plain",
                "Cache-Control": "no-store",
            },
        })
    } catch (error) {
        return new NextResponse(
            "Failed to connect to backend",
            { status: 502 }
        )
    }
}
