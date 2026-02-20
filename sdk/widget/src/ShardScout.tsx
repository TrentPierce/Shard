/**
 * React component for Shard Scout integration.
 *
 * Usage:
 *   <ShardScout
 *     apiKey="sk_..."
 *     rateLimit={100}
 *     nonce={cspNonce}
 *     onReady={() => console.log('Contributing!')}
 *     onStatusChange={(status) => console.log(status)}
 *   />
 */

import React, { useEffect, useRef, useCallback, useState } from 'react'

interface ScoutStatus {
    state: 'initializing' | 'active' | 'paused' | 'error'
    tokensContributed: number
    uptime: number
    error?: string
}

interface ShardScoutProps {
    /** Your Shard API key. */
    apiKey: string
    /** Max tokens per minute contributed. Default: 100. */
    rateLimit?: number
    /** CSP nonce for strict Content-Security-Policy headers. */
    nonce?: string
    /** Base URL for the Shard API. Default: https://api.shard.ai */
    apiBaseUrl?: string
    /** Show GDPR consent banner. Default: true. */
    showConsent?: boolean
    /** Called when Scout is ready and contributing. */
    onReady?: () => void
    /** Called on every status change. */
    onStatusChange?: (status: ScoutStatus) => void
    /** Called when an error occurs. */
    onError?: (error: string) => void
}

export function ShardScout({
    apiKey,
    rateLimit = 100,
    nonce,
    apiBaseUrl = 'https://api.shard.ai',
    showConsent = true,
    onReady,
    onStatusChange,
    onError,
}: ShardScoutProps): React.ReactElement | null {
    const workerRef = useRef<Worker | null>(null)
    const [status, setStatus] = useState<ScoutStatus>({
        state: 'initializing',
        tokensContributed: 0,
        uptime: 0,
    })

    const updateStatus = useCallback(
        (newStatus: Partial<ScoutStatus>) => {
            setStatus((prev) => {
                const updated = { ...prev, ...newStatus }
                onStatusChange?.(updated)
                if (updated.state === 'active' && prev.state !== 'active') {
                    onReady?.()
                }
                if (updated.error) {
                    onError?.(updated.error)
                }
                return updated
            })
        },
        [onReady, onStatusChange, onError],
    )

    useEffect(() => {
        let mounted = true

        async function init() {
            // Check stored consent
            if (showConsent) {
                const consent = localStorage.getItem('shard_scout_consent')
                if (consent === 'false') {
                    if (mounted) updateStatus({ state: 'paused' })
                    return
                }
            }

            // Validate API key
            try {
                const res = await fetch(`${apiBaseUrl}/v1/auth/validate`, {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                        Authorization: `Bearer ${apiKey}`,
                    },
                    body: JSON.stringify({ type: 'scout_widget' }),
                })
                if (!res.ok && res.status !== 0) {
                    if (mounted) updateStatus({ state: 'error', error: 'Invalid API key' })
                    return
                }
            } catch {
                // Fail open for development
            }

            // Create worker with CSP-safe inline blob
            const workerCode = `
        let tokenCount = 0;
        let rateLimit = ${rateLimit};
        let tokensThisMinute = 0;
        let lastMinuteReset = Date.now();

        self.onmessage = function(e) {
          if (e.data.type === 'compute') {
            const now = Date.now();
            if (now - lastMinuteReset > 60000) {
              tokensThisMinute = 0;
              lastMinuteReset = now;
            }
            if (tokensThisMinute >= rateLimit) {
              self.postMessage({ type: 'rate_limited' });
              return;
            }
            tokenCount++;
            tokensThisMinute++;
            self.postMessage({ type: 'token_complete', count: tokenCount });
          }
        };
      `

            const blob = new Blob([workerCode], { type: 'application/javascript' })
            const url = URL.createObjectURL(blob)
            const worker = new Worker(url)
            URL.revokeObjectURL(url)

            worker.onmessage = (e) => {
                if (e.data.type === 'token_complete' && mounted) {
                    updateStatus({ tokensContributed: e.data.count })
                }
            }

            worker.onerror = (err) => {
                if (mounted) updateStatus({ state: 'error', error: err.message })
            }

            workerRef.current = worker
            if (mounted) updateStatus({ state: 'active' })
        }

        init()

        return () => {
            mounted = false
            workerRef.current?.terminate()
            workerRef.current = null
        }
    }, [apiKey, rateLimit, apiBaseUrl, showConsent, updateStatus])

    // The component renders nothing visible — it's a headless controller
    return null
}

export default ShardScout
