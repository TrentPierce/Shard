import { NextResponse } from "next/server"
import type { NextRequest } from "next/server"

const DEFAULT_CONNECT_SOURCES = [
  "'self'",
  "https:",
  "wss:",
  "http://127.0.0.1:*",
  "http://localhost:*",
  "ws://127.0.0.1:*",
  "ws://localhost:*",
]

const buildConnectSources = () => {
  const extraOrigins = (process.env.NEXT_PUBLIC_CONNECT_SRC_ALLOWLIST ?? "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean)

  return [...DEFAULT_CONNECT_SOURCES, ...extraOrigins].join(" ")
}

export function middleware(request: NextRequest) {
  const isDev = process.env.NODE_ENV !== "production"

  const cspHeader = `
    default-src 'self';
    script-src 'self' ${isDev ? "'unsafe-eval'" : ""} 'unsafe-inline' blob:;
    style-src 'self' 'unsafe-inline' https://fonts.googleapis.com;
    font-src 'self' data: https://fonts.gstatic.com;
    connect-src ${buildConnectSources()};
    img-src 'self' data: blob: https:;
    worker-src 'self' blob:;
    child-src 'self' blob:;
    object-src 'none';
    base-uri 'self';
    form-action 'self';
    frame-ancestors 'none';
    upgrade-insecure-requests;
  `
    .replace(/\s{2,}/g, " ")
    .trim()

  const response = NextResponse.next()

  response.headers.set("Content-Security-Policy", cspHeader)

  return response
}

export const config = {
  matcher: [
    {
      source: "/((?!api|_next/static|_next/image|favicon.ico).*)",
      missing: [
        { type: "header", key: "next-router-prefetch" },
        { type: "header", key: "purpose", value: "prefetch" },
      ],
    },
  ],
}
