import { headers } from "next/headers"

const DEFAULT_BACKEND = "http://35.175.242.222:9091"

export function getShardBackendBaseUrl(): string {
  return (
    process.env.SHARD_BACKEND_URL?.trim() ||
    process.env.NEXT_PUBLIC_SHARD_BACKEND_URL?.trim() ||
    DEFAULT_BACKEND
  ).replace(/\/$/, "")
}

export function shardBackendUrl(path: string): string {
  const cleanPath = path.startsWith("/") ? path : `/${path}`
  return `${getShardBackendBaseUrl()}${cleanPath}`
}

export function forwardRequestHeaders(contentType = "application/json"): HeadersInit {
  const incoming = headers()
  const auth = incoming.get("authorization")
  const wallet = incoming.get("x-shard-wallet")
  const inferenceMode = incoming.get("x-shard-inference-mode")

  const out: Record<string, string> = { "Content-Type": contentType }
  if (auth) out.Authorization = auth
  if (wallet) out["X-Shard-Wallet"] = wallet
  if (inferenceMode) out["X-Shard-Inference-Mode"] = inferenceMode
  return out
}
