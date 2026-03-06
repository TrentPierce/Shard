const ALLOWED_OWNERS = new Set(["mlc-ai"])
const ALLOWED_REPOS = new Set([
  "Llama-3.2-1B-Instruct-q4f32_1-MLC",
  "Llama-3.2-1B-Instruct-q4f16_1-MLC",
  "TinyLlama-1.1B-Chat-v1.0-q4f32_1-MLC",
  "TinyLlama-1.1B-Chat-v1.0-q4f16_1-MLC",
])

export const runtime = "edge"
export const dynamic = "force-dynamic"

type RouteContext = {
  params: Promise<{
    owner: string
    repo: string
    asset?: string[]
  }>
}

function badRequest(message: string, status = 400): Response {
  return Response.json({ ok: false, detail: message }, { status })
}

function copyHeaderIfPresent(target: Headers, source: Headers, name: string): void {
  const value = source.get(name)
  if (value) {
    target.set(name, value)
  }
}

async function handleProxy(request: Request, context: RouteContext): Promise<Response> {
  const { owner, repo, asset = [] } = await context.params
  const normalizedOwner = String(owner || "").trim()
  const normalizedRepo = String(repo || "").trim()
  const normalizedAsset = asset.map((segment) => String(segment || "").trim()).filter(Boolean)

  if (!ALLOWED_OWNERS.has(normalizedOwner) || !ALLOWED_REPOS.has(normalizedRepo)) {
    return badRequest("webllm_model_not_allowed", 404)
  }
  if (normalizedAsset.length < 3 || normalizedAsset[0] !== "resolve" || normalizedAsset[1] !== "main") {
    return badRequest("webllm_asset_path_invalid", 404)
  }

  const upstreamUrl = new URL(
    `https://huggingface.co/${normalizedOwner}/${normalizedRepo}/${normalizedAsset.join("/")}`,
  )
  const upstreamHeaders = new Headers()
  copyHeaderIfPresent(upstreamHeaders, request.headers, "range")
  copyHeaderIfPresent(upstreamHeaders, request.headers, "if-none-match")
  copyHeaderIfPresent(upstreamHeaders, request.headers, "if-modified-since")
  copyHeaderIfPresent(upstreamHeaders, request.headers, "accept")

  const upstream = await fetch(upstreamUrl, {
    method: request.method,
    headers: upstreamHeaders,
    redirect: "follow",
    cache: "force-cache",
  })

  const responseHeaders = new Headers()
  const passthroughHeaders = [
    "content-type",
    "content-length",
    "content-disposition",
    "cache-control",
    "etag",
    "last-modified",
    "accept-ranges",
    "content-range",
  ]
  for (const header of passthroughHeaders) {
    copyHeaderIfPresent(responseHeaders, upstream.headers, header)
  }
  responseHeaders.set("Access-Control-Allow-Origin", "*")
  responseHeaders.set("Cross-Origin-Resource-Policy", "cross-origin")
  responseHeaders.set("X-Shard-WebLLM-Proxy", "huggingface")

  if (request.method === "HEAD") {
    return new Response(null, {
      status: upstream.status,
      headers: responseHeaders,
    })
  }

  return new Response(upstream.body, {
    status: upstream.status,
    headers: responseHeaders,
  })
}

export async function GET(request: Request, context: RouteContext): Promise<Response> {
  return handleProxy(request, context)
}

export async function HEAD(request: Request, context: RouteContext): Promise<Response> {
  return handleProxy(request, context)
}
