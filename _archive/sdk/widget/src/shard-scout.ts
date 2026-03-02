/**
 * Shard Scout Widget — drop-in <script> tag for browser Scout contribution.
 *
 * Usage:
 *   <script src="https://cdn.shard.ai/scout.js"
 *           integrity="sha384-{hash}"
 *           crossorigin="anonymous"
 *           data-api-key="sk_..."
 *           data-rate-limit="100"></script>
 *
 * The widget:
 * 1. Validates the API key against the Shard API
 * 2. Initializes a Web Worker for off-main-thread Scout compute
 * 3. Optionally shows a GDPR consent banner
 * 4. Reports contribution metrics back to the host page via CustomEvents
 */

interface ScoutWidgetConfig {
    apiKey: string
    rateLimit: number
    consent: boolean
    apiBaseUrl: string
    nonce?: string
}

interface ScoutStatus {
    state: 'initializing' | 'active' | 'paused' | 'error'
    tokensContributed: number
    uptime: number
    error?: string
}

// ─── Auto-Initialize from <script> data attributes ─────────────────

function autoInit(): void {
    const scriptEl = document.currentScript as HTMLScriptElement | null
    if (!scriptEl) return

    const apiKey = scriptEl.dataset.apiKey
    if (!apiKey) {
        console.error('[Shard Scout] Missing data-api-key attribute')
        return
    }

    const config: ScoutWidgetConfig = {
        apiKey,
        rateLimit: parseInt(scriptEl.dataset.rateLimit ?? '100', 10),
        consent: scriptEl.dataset.consent !== 'false',
        apiBaseUrl: scriptEl.dataset.apiUrl ?? 'https://api.shard.ai',
        nonce: scriptEl.nonce || scriptEl.dataset.nonce,
    }

    initScoutWidget(config)
}

// ─── Widget Core ────────────────────────────────────────────────────

let scoutWorker: Worker | null = null
let scoutStatus: ScoutStatus = {
    state: 'initializing',
    tokensContributed: 0,
    uptime: 0,
}
let startTime = 0

async function initScoutWidget(config: ScoutWidgetConfig): Promise<void> {
    try {
        // 1. Validate API key
        const valid = await validateApiKey(config)
        if (!valid) {
            updateStatus('error', 'Invalid API key')
            return
        }

        // 2. Show consent banner if configured
        if (config.consent) {
            const consented = await showConsentBanner(config.nonce)
            if (!consented) {
                updateStatus('paused', 'User declined consent')
                return
            }
        }

        // 3. Initialize Scout Web Worker
        startTime = Date.now()
        scoutWorker = createScoutWorker(config)
        updateStatus('active')

        // 4. Start uptime counter
        setInterval(() => {
            if (scoutStatus.state === 'active') {
                scoutStatus.uptime = Math.floor((Date.now() - startTime) / 1000)
                dispatchStatusEvent()
            }
        }, 5000)
    } catch (err) {
        updateStatus('error', (err as Error).message)
    }
}

async function validateApiKey(config: ScoutWidgetConfig): Promise<boolean> {
    try {
        const response = await fetch(`${config.apiBaseUrl}/v1/auth/validate`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                Authorization: `Bearer ${config.apiKey}`,
            },
            body: JSON.stringify({ type: 'scout_widget' }),
        })
        return response.ok
    } catch {
        // If API is unreachable, allow initialization (fail-open for dev)
        console.warn('[Shard Scout] API key validation failed — proceeding anyway')
        return true
    }
}

function createScoutWorker(config: ScoutWidgetConfig): Worker {
    // Inline worker script to avoid CSP issues with external worker URLs
    const workerCode = `
    let tokenCount = 0;
    let rateLimit = ${config.rateLimit};
    let tokensThisMinute = 0;
    let lastMinuteReset = Date.now();

    self.onmessage = function(e) {
      if (e.data.type === 'compute') {
        // Rate limiting
        const now = Date.now();
        if (now - lastMinuteReset > 60000) {
          tokensThisMinute = 0;
          lastMinuteReset = now;
        }
        if (tokensThisMinute >= rateLimit) {
          self.postMessage({ type: 'rate_limited' });
          return;
        }
        
        // Simulate Scout compute (actual WebLLM integration goes here)
        tokenCount++;
        tokensThisMinute++;
        self.postMessage({ type: 'token_complete', count: tokenCount });
      }
      if (e.data.type === 'status') {
        self.postMessage({ type: 'status', tokenCount, tokensThisMinute });
      }
    };
  `

    const blob = new Blob([workerCode], { type: 'application/javascript' })
    const url = URL.createObjectURL(blob)
    const worker = new Worker(url)

    worker.onmessage = (e) => {
        if (e.data.type === 'token_complete') {
            scoutStatus.tokensContributed = e.data.count
            dispatchStatusEvent()
        }
    }

    worker.onerror = (err) => {
        updateStatus('error', err.message)
    }

    // Clean up blob URL
    URL.revokeObjectURL(url)

    return worker
}

// ─── Consent Banner ─────────────────────────────────────────────────

function showConsentBanner(nonce?: string): Promise<boolean> {
    return new Promise((resolve) => {
        const banner = document.createElement('div')
        banner.id = 'shard-consent-banner'

        // Apply nonce for CSP compliance
        if (nonce) {
            banner.setAttribute('nonce', nonce)
        }

        banner.innerHTML = `
      <div style="
        position: fixed; bottom: 0; left: 0; right: 0;
        background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
        color: #e0e0e0; padding: 16px 24px;
        display: flex; align-items: center; justify-content: space-between;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
        font-size: 14px; z-index: 999999;
        box-shadow: 0 -4px 20px rgba(0,0,0,0.3);
        border-top: 1px solid rgba(255,255,255,0.1);
      ">
        <span>
          🧠 This site uses <strong>Shard</strong> to contribute spare compute
          to distributed AI inference. No personal data is collected.
          <a href="https://shard.ai/privacy" target="_blank" rel="noopener"
             style="color: #7c83ff; text-decoration: underline;">Learn more</a>
        </span>
        <div style="display: flex; gap: 8px; margin-left: 16px; flex-shrink: 0;">
          <button id="shard-consent-accept" style="
            background: #7c83ff; color: white; border: none; padding: 8px 20px;
            border-radius: 6px; cursor: pointer; font-size: 13px; font-weight: 600;
          ">Accept</button>
          <button id="shard-consent-decline" style="
            background: transparent; color: #999; border: 1px solid #555;
            padding: 8px 20px; border-radius: 6px; cursor: pointer; font-size: 13px;
          ">Decline</button>
        </div>
      </div>
    `

        document.body.appendChild(banner)

        document.getElementById('shard-consent-accept')!.onclick = () => {
            banner.remove()
            localStorage.setItem('shard_scout_consent', 'true')
            resolve(true)
        }

        document.getElementById('shard-consent-decline')!.onclick = () => {
            banner.remove()
            localStorage.setItem('shard_scout_consent', 'false')
            resolve(false)
        }

        // Check for existing consent
        const stored = localStorage.getItem('shard_scout_consent')
        if (stored === 'true') {
            banner.remove()
            resolve(true)
        } else if (stored === 'false') {
            banner.remove()
            resolve(false)
        }
    })
}

// ─── Status Management ──────────────────────────────────────────────

function updateStatus(state: ScoutStatus['state'], error?: string): void {
    scoutStatus.state = state
    if (error) scoutStatus.error = error
    dispatchStatusEvent()
}

function dispatchStatusEvent(): void {
    window.dispatchEvent(
        new CustomEvent('shard:scout:status', {
            detail: { ...scoutStatus },
        }),
    )
}

// ─── Public API (attached to window) ────────────────────────────────

declare global {
    interface Window {
        ShardScout: {
            init: (config: ScoutWidgetConfig) => Promise<void>
            pause: () => void
            resume: () => void
            getStatus: () => ScoutStatus
            destroy: () => void
        }
    }
}

window.ShardScout = {
    init: initScoutWidget,
    pause: () => {
        updateStatus('paused')
    },
    resume: () => {
        if (scoutWorker) updateStatus('active')
    },
    getStatus: () => ({ ...scoutStatus }),
    destroy: () => {
        if (scoutWorker) {
            scoutWorker.terminate()
            scoutWorker = null
        }
        updateStatus('paused')
    },
}

// Auto-init when loaded via <script> tag
if (typeof document !== 'undefined' && document.currentScript) {
    autoInit()
}

export type { ScoutWidgetConfig, ScoutStatus }
