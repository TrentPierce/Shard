export const runtime = "edge"
export const dynamic = "force-dynamic"

import { NextRequest } from "next/server"
import { proxyOptions, proxyShardJsonPost } from "@/lib/server/shard-json-proxy"

export async function POST(request: NextRequest) {
  return proxyShardJsonPost(request, "/v1/agents/tasks")
}

export async function OPTIONS(request: NextRequest) {
  return proxyOptions(request, "POST, OPTIONS")
}
