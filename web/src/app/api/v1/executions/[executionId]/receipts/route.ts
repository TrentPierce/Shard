export const runtime = "edge"
export const dynamic = "force-dynamic"

import { NextRequest } from "next/server"
import { proxyOptions, proxyShardJsonGet } from "@/lib/server/shard-json-proxy"

export async function GET(
  request: NextRequest,
  context: { params: { executionId: string } },
) {
  return proxyShardJsonGet(
    request,
    `/v1/executions/${context.params.executionId}/receipts`,
  )
}

export async function OPTIONS(request: NextRequest) {
  return proxyOptions(request, "GET, OPTIONS")
}
