export const runtime = "edge"
export const dynamic = "force-dynamic"

import { NextRequest } from "next/server"
import { proxyShardJsonGet } from "@/lib/server/shard-json-proxy"

export async function GET(request: NextRequest) {
  return proxyShardJsonGet(request, "/v1/capabilities")
}
