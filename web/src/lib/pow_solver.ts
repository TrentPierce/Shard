/**
 * Proof-of-Work Solver Web Worker
 *
 * Brute-forces a Hashcash nonce using crypto.subtle.digest('SHA-256', ...).
 * Runs off the main thread to avoid blocking UI and inference.
 *
 * Hardware-aware throttling:
 * - On mobile (≤3 cores), runs at 50% duty cycle to avoid battery drain
 *   and browser cryptominer flags (compute 100ms, yield 100ms).
 * - Aborts and signals captcha fallback if solve exceeds 5s.
 *
 * Usage:
 *   const worker = new Worker(new URL('./pow_solver.ts', import.meta.url));
 *   worker.postMessage({ challengeHex, difficulty, hardwareConcurrency });
 *   worker.onmessage = (e) => {
 *     if (e.data.type === 'solved') { ... }
 *     if (e.data.type === 'timeout') { // fall back to captcha }
 *     if (e.data.type === 'progress') { // optional UI update }
 *   };
 */

interface PowWorkerRequest {
    challengeHex: string
    difficulty: number
    hardwareConcurrency: number
}

interface PowWorkerSolved {
    type: 'solved'
    nonce: number
    hashHex: string
    elapsedMs: number
}

interface PowWorkerTimeout {
    type: 'timeout'
    elapsedMs: number
    attemptedNonces: number
}

interface PowWorkerProgress {
    type: 'progress'
    attemptedNonces: number
    elapsedMs: number
}

type PowWorkerResponse = PowWorkerSolved | PowWorkerTimeout | PowWorkerProgress

const MAX_SOLVE_TIME_MS = 5000
const BATCH_SIZE = 1024
const MOBILE_COMPUTE_MS = 100
const MOBILE_YIELD_MS = 100
const PROGRESS_INTERVAL = 10000 // report every 10k nonces

function hexToBytes(hex: string): Uint8Array {
    const bytes = new Uint8Array(hex.length / 2)
    for (let i = 0; i < hex.length; i += 2) {
        bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16)
    }
    return bytes
}

function bytesToHex(bytes: Uint8Array): string {
    return Array.from(bytes)
        .map((b) => b.toString(16).padStart(2, '0'))
        .join('')
}

function countLeadingZeroBits(hash: Uint8Array): number {
    let count = 0
    for (const byte of hash) {
        if (byte === 0) {
            count += 8
        } else {
            // Count leading zeros in this byte
            let b = byte
            while ((b & 0x80) === 0 && count < hash.length * 8) {
                count++
                b <<= 1
            }
            break
        }
    }
    return count
}

function uint64ToLeBytes(n: number): Uint8Array {
    const buf = new ArrayBuffer(8)
    const view = new DataView(buf)
    // JavaScript numbers are doubles, safe up to 2^53
    view.setUint32(0, n & 0xffffffff, true) // little-endian low 32 bits
    view.setUint32(4, Math.floor(n / 0x100000000) & 0xffffffff, true) // high 32 bits
    return new Uint8Array(buf)
}

async function solvePow(
    challengeBytes: Uint8Array,
    difficulty: number,
    isMobile: boolean,
): Promise<void> {
    const startTime = performance.now()
    let nonce = 0
    let lastProgressReport = 0

    while (true) {
        const elapsed = performance.now() - startTime

        // Timeout check
        if (elapsed > MAX_SOLVE_TIME_MS) {
            const msg: PowWorkerTimeout = {
                type: 'timeout',
                elapsedMs: Math.round(elapsed),
                attemptedNonces: nonce,
            }
            self.postMessage(msg)
            return
        }

        // Process a batch of nonces
        const batchEnd = nonce + BATCH_SIZE
        for (; nonce < batchEnd; nonce++) {
            const nonceBytes = uint64ToLeBytes(nonce)
            const input = new Uint8Array(challengeBytes.length + 8)
            input.set(challengeBytes, 0)
            input.set(nonceBytes, challengeBytes.length)

            const hashBuffer = await crypto.subtle.digest('SHA-256', input)
            const hashArray = new Uint8Array(hashBuffer)
            const leadingZeros = countLeadingZeroBits(hashArray)

            if (leadingZeros >= difficulty) {
                const msg: PowWorkerSolved = {
                    type: 'solved',
                    nonce,
                    hashHex: bytesToHex(hashArray),
                    elapsedMs: Math.round(performance.now() - startTime),
                }
                self.postMessage(msg)
                return
            }
        }

        // Progress reporting
        if (nonce - lastProgressReport >= PROGRESS_INTERVAL) {
            const msg: PowWorkerProgress = {
                type: 'progress',
                attemptedNonces: nonce,
                elapsedMs: Math.round(performance.now() - startTime),
            }
            self.postMessage(msg)
            lastProgressReport = nonce
        }

        // Mobile throttling: yield for MOBILE_YIELD_MS every MOBILE_COMPUTE_MS
        if (isMobile) {
            const computeElapsed = performance.now() - startTime
            if (computeElapsed % (MOBILE_COMPUTE_MS + MOBILE_YIELD_MS) > MOBILE_COMPUTE_MS) {
                await new Promise((resolve) => setTimeout(resolve, MOBILE_YIELD_MS))
            }
        }
    }
}

self.onmessage = async (event: MessageEvent<PowWorkerRequest>) => {
    const { challengeHex, difficulty, hardwareConcurrency } = event.data
    const challengeBytes = hexToBytes(challengeHex)
    const isMobile = hardwareConcurrency <= 3

    await solvePow(challengeBytes, difficulty, isMobile)
}
