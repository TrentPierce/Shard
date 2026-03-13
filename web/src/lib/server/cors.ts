import type { NextRequest } from "next/server"

function parseCorsOrigins(raw: string | undefined): string[] {
  return (raw ?? "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean)
}

const CORS_ALLOWLIST = parseCorsOrigins(process.env.SHARD_CORS_ORIGINS)
const CORS_ALLOWLIST_SET = new Set(CORS_ALLOWLIST)

export function resolveCorsOrigin(
  request: Pick<NextRequest, "headers" | "nextUrl">,
): string | null {
  const origin = request.headers.get("origin")
  if (!origin) {
    return null
  }

  const forwardedHost = request.headers.get("x-forwarded-host") || request.headers.get("host")
  const forwardedProto = request.headers.get("x-forwarded-proto") || "https"
  if (forwardedHost && origin === `${forwardedProto}://${forwardedHost}`) {
    return origin
  }

  const siteOrigin = process.env.NEXT_PUBLIC_SITE_URL?.trim().replace(/\/$/, "")
  if (siteOrigin && origin === siteOrigin) {
    return origin
  }

  if (origin === request.nextUrl.origin) {
    return origin
  }

  if (CORS_ALLOWLIST_SET.has("*") || CORS_ALLOWLIST_SET.has(origin)) {
    return origin
  }

  return null
}

export function corsHeadersForRequest(
  request: Pick<NextRequest, "headers" | "nextUrl">,
): Record<string, string> {
  const origin = resolveCorsOrigin(request)
  if (!origin) {
    return {}
  }

  return {
    "Access-Control-Allow-Origin": origin,
    Vary: "Origin",
  }
}

export function buildPreflightResponse(
  request: Pick<NextRequest, "headers" | "nextUrl">,
  methods: string,
) {
  const origin = resolveCorsOrigin(request)
  if (!origin) {
    return new Response(null, { status: 403 })
  }

  return new Response(null, {
    status: 204,
    headers: {
      "Access-Control-Allow-Origin": origin,
      "Access-Control-Allow-Methods": methods,
      "Access-Control-Allow-Headers":
        "Content-Type, Authorization, X-Shard-Inference-Mode, X-Shard-Wallet, X-Shard-Backend-Url",
      Vary: "Origin",
    },
  })
}
