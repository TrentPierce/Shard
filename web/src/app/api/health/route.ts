import { NextRequest, NextResponse } from "next/server"

const EC2_URL = "http://35.175.242.222:9091"

export async function GET(request: NextRequest) {
  const url = `${EC2_URL}/health`
  
  try {
    const response = await fetch(url, {
      signal: AbortSignal.timeout(8000),
    })
    const data = await response.json()
    return NextResponse.json(data)
  } catch (error) {
    return NextResponse.json({
      error: "Failed to connect to backend",
      details: String(error),
    }, { status: 502 })
  }
}
