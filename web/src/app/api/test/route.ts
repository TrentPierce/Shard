import { NextRequest, NextResponse } from "next/server"

export async function POST(request: NextRequest) {
  const body = await request.json()
  return NextResponse.json({ 
    success: true, 
    received: body,
    message: "POST works!" 
  })
}

export async function GET() {
  return NextResponse.json({ 
    success: true, 
    message: "GET works!" 
  })
}
