
import { NextResponse } from "next/server"
import { getChatProxySliSnapshot } from "@/lib/server/proxy-chat-sli"

export const dynamic = "force-dynamic"

export async function GET() {
  return NextResponse.json({
    status: "ok",
    proxy_chat_sli: getChatProxySliSnapshot(),
  })
}

