import { NextResponse } from "next/server"
import { fetchWithBackendFailover } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET() {
    try {
        const { response } = await fetchWithBackendFailover("/metrics")
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
