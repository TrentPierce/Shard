import { NextResponse } from "next/server"

export const dynamic = "force-dynamic"

export async function GET() {
  // Test EC2 connectivity
  try {
    const response = await fetch("http://35.175.242.222:9091/health", {
      signal: AbortSignal.timeout(5000),
    })
    const data = await response.json()
    return NextResponse.json({
      status: "ok",
      ec2_status: data.status,
      ec2_version: data.version,
      ec2_peer_id: data.peer_id,
    })
  } catch (error) {
    return NextResponse.json({
      status: "error",
      error: String(error),
    }, { status: 200 })
  }
}
