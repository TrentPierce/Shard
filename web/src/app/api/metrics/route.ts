
import { NextResponse } from "next/server"
import { fetchWithBackendFailover } from "@/lib/server/shard-backend"
import { getChatProxySliSnapshot } from "@/lib/server/proxy-chat-sli"

export const dynamic = "force-dynamic"

export async function GET() {
    try {
        const [{ response }, health] = await Promise.all([
            fetchWithBackendFailover("/metrics"),
            fetchWithBackendFailover("/health", {
                timeoutMs: 2_500,
                totalTimeoutMs: 4_000,
                maxAttempts: 2,
                failoverOnStatuses: [500, 502, 503, 504],
            }).catch(() => null),
        ])
        const text = await response.text()
        const chatSli = getChatProxySliSnapshot()
        const healthPayload = health
            ? await health.response.clone().json().catch(() => ({}))
            : {}
        const healthObj =
            healthPayload && typeof healthPayload === "object"
                ? (healthPayload as Record<string, unknown>)
                : {}
        const rawHealthState = String(
            healthObj.health_state ?? healthObj.status ?? "",
        ).toLowerCase()
        const backendHealthReady =
            rawHealthState === "ready" ||
            (rawHealthState === "ok" &&
                healthObj.ready_for_inference !== false)
                ? 1
                : 0
        const extraLines = [
            "# HELP shard_web_proxy_backend_health_ready Backend health state observed by web proxy (1=ready,0=not-ready).",
            "# TYPE shard_web_proxy_backend_health_ready gauge",
            `shard_web_proxy_backend_health_ready ${backendHealthReady}`,
            "# HELP shard_web_chat_proxy_requests_total Total proxied /v1/chat/completions requests observed by web.",
            "# TYPE shard_web_chat_proxy_requests_total counter",
            `shard_web_chat_proxy_requests_total ${chatSli.counters.requests_total}`,
            "# HELP shard_web_chat_proxy_success_total Successful proxied /v1/chat/completions requests.",
            "# TYPE shard_web_chat_proxy_success_total counter",
            `shard_web_chat_proxy_success_total ${chatSli.counters.success_total}`,
            "# HELP shard_web_chat_proxy_http_5xx_total Proxied /v1/chat/completions requests that ended in upstream 5xx.",
            "# TYPE shard_web_chat_proxy_http_5xx_total counter",
            `shard_web_chat_proxy_http_5xx_total ${chatSli.counters.http_5xx_total}`,
            "# HELP shard_web_chat_proxy_timeout_total Proxied /v1/chat/completions requests that timed out.",
            "# TYPE shard_web_chat_proxy_timeout_total counter",
            `shard_web_chat_proxy_timeout_total ${chatSli.counters.timeout_total}`,
            "# HELP shard_web_chat_proxy_other_error_total Proxied /v1/chat/completions requests with non-timeout/non-5xx errors.",
            "# TYPE shard_web_chat_proxy_other_error_total counter",
            `shard_web_chat_proxy_other_error_total ${chatSli.counters.other_error_total}`,
            "# HELP shard_web_chat_proxy_requests_5m Requests in the current 5m chat proxy SLI window.",
            "# TYPE shard_web_chat_proxy_requests_5m gauge",
            `shard_web_chat_proxy_requests_5m ${chatSli.window_5m.request_count}`,
            "# HELP shard_web_chat_proxy_http_5xx_rate_5m HTTP 5xx ratio in the current 5m chat proxy SLI window.",
            "# TYPE shard_web_chat_proxy_http_5xx_rate_5m gauge",
            `shard_web_chat_proxy_http_5xx_rate_5m ${chatSli.window_5m.http_5xx_rate}`,
            "# HELP shard_web_chat_proxy_timeout_rate_5m Timeout ratio in the current 5m chat proxy SLI window.",
            "# TYPE shard_web_chat_proxy_timeout_rate_5m gauge",
            `shard_web_chat_proxy_timeout_rate_5m ${chatSli.window_5m.timeout_rate}`,
            "# HELP shard_web_chat_proxy_error_rate_5m Overall error ratio in the current 5m chat proxy SLI window.",
            "# TYPE shard_web_chat_proxy_error_rate_5m gauge",
            `shard_web_chat_proxy_error_rate_5m ${chatSli.window_5m.error_rate}`,
        ].join("\n")
        const joined = `${text.trimEnd()}\n${extraLines}\n`

        return new NextResponse(joined, {
            status: response.status,
            headers: {
                "Content-Type": "text/plain",
                "Cache-Control": "no-store",
            },
        })
    } catch (error) {
        return new NextResponse(
            "Failed to connect to backend",
            { status: 502 }
        )
    }
}

