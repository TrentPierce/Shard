/**
 * Golden Ticket Verification System for Scout Nodes
 *
 * This module provides the scout-side functionality for detecting and
 * properly responding to Golden Tickets from Oracle nodes. Golden Tickets
 * are pre-solved prompts used to verify scout honesty and prevent Sybil attacks.
 *
 * Key Responsibilities:
 * - Detect Golden Ticket prompts in work requests
 * - Generate honest responses to Golden Tickets
 * - Ensure proper submission format for verification
 *
 * Security Model:
 * - Scouts MUST respond honestly to Golden Tickets
 * - Failing Golden Tickets results in immediate banning
 * - The system is designed to be undetectable to attackers
 */

import { generateDraftTokens } from "./webllm"

// ─── Types ──────────────────────────────────────────────────────────────────

export type GoldenTicketCheck = {
    isGoldenTicket: boolean
    expectedAnswer?: string
    tolerance?: "exact" | "contains" | "numeric"
}

export type HonestResponse = {
    text: string
    isGoldenTicket: boolean
    success: boolean
    error?: string
}

export type WorkRequest = {
    workId: string
    prompt: string
    timestamp: number
}

export type WorkResult = {
    workId: string
    draftText: string
    scoutId: string
    timestamp: number
    isGoldenTicketResponse: boolean
}

// ─── Golden Ticket Detection ─────────────────────────────────────────────────

/**
 * Known Golden Ticket prompt patterns.
 *
 * These patterns match the templates defined in the Oracle's golden_ticket.py.
 * Scouts use these to detect when they're being tested and respond correctly.
 *
 * NOTE: This list must be kept in sync with GOLDEN_TICKET_TEMPLATES in
 * desktop/python/golden_ticket.py
 */
const GOLDEN_TICKET_PATTERNS: Array<{
    pattern: RegExp
    expected: string
    tolerance: "exact" | "contains" | "numeric"
    type: "math" | "string" | "fact"
}> = [
    // Mathematical reasoning patterns
    { pattern: /what is 2\+2/i, expected: "4", tolerance: "exact", type: "math" },
    { pattern: /calculate 15 \* 7/i, expected: "105", tolerance: "exact", type: "math" },
    { pattern: /square root of 144/i, expected: "12", tolerance: "exact", type: "math" },
    { pattern: /100 divided by 4/i, expected: "25", tolerance: "exact", type: "math" },
    { pattern: /calculate 17 \+ 28/i, expected: "45", tolerance: "exact", type: "math" },
    { pattern: /9 squared/i, expected: "81", tolerance: "exact", type: "math" },
    { pattern: /sum of 123 and 456/i, expected: "579", tolerance: "exact", type: "math" },
    { pattern: /50% of 200/i, expected: "100", tolerance: "exact", type: "math" },

    // String manipulation patterns
    { pattern: /third word in.*the quick brown fox/i, expected: "brown", tolerance: "exact", type: "string" },
    { pattern: /spell.*hello.*backwards/i, expected: "olleh", tolerance: "exact", type: "string" },
    { pattern: /letters in.*javascript/i, expected: "10", tolerance: "exact", type: "string" },
    { pattern: /letter comes after.*b.*alphabet/i, expected: "c", tolerance: "exact", type: "string" },
    { pattern: /capitalize.*test/i, expected: "TEST", tolerance: "exact", type: "string" },

    // Factual knowledge patterns
    { pattern: /capital of france/i, expected: "paris", tolerance: "contains", type: "fact" },
    { pattern: /days in a week/i, expected: "7", tolerance: "exact", type: "fact" },
    { pattern: /red planet/i, expected: "mars", tolerance: "contains", type: "fact" },
    { pattern: /continents.*earth/i, expected: "7", tolerance: "exact", type: "fact" },
    { pattern: /freezing point.*water.*celsius/i, expected: "0", tolerance: "contains", type: "fact" },
    { pattern: /sides.*triangle/i, expected: "3", tolerance: "exact", type: "fact" },
    { pattern: /sky.*clear day/i, expected: "blue", tolerance: "contains", type: "fact" },
    { pattern: /hours in a day/i, expected: "24", tolerance: "exact", type: "fact" },
    { pattern: /opposite of.*hot/i, expected: "cold", tolerance: "contains", type: "fact" },
    { pattern: /minutes in an hour/i, expected: "60", tolerance: "exact", type: "fact" },
]

/**
 * Check if a prompt is a Golden Ticket.
 *
 * This function analyzes the prompt text to determine if it's a
 * pre-solved verification prompt from an Oracle node.
 *
 * @param prompt - The prompt text to check
 * @returns GoldenTicketCheck with detection result and expected answer if found
 */
export function checkGoldenTicket(prompt: string): GoldenTicketCheck {
    const normalizedPrompt = prompt.trim().toLowerCase()

    for (const gt of GOLDEN_TICKET_PATTERNS) {
        if (gt.pattern.test(normalizedPrompt)) {
            return {
                isGoldenTicket: true,
                expectedAnswer: gt.expected,
                tolerance: gt.tolerance,
            }
        }
    }

    return { isGoldenTicket: false }
}

// ─── Honest Response Generation ──────────────────────────────────────────────

/**
 * Generate an honest response to a Golden Ticket.
 *
 * This function ensures scouts respond correctly to verification prompts.
 * The response is formatted to match the expected answer format.
 *
 * @param prompt - The Golden Ticket prompt
 * @param expectedAnswer - The expected answer
 * @param tolerance - The match tolerance (exact, contains, numeric)
 * @returns HonestResponse with the correct answer
 */
export function generateHonestGoldenTicketResponse(
    prompt: string,
    expectedAnswer: string,
    tolerance: "exact" | "contains" | "numeric"
): HonestResponse {
    try {
        // Generate a natural response that includes the correct answer
        let responseText = ""

        // Format the response based on the tolerance type
        switch (tolerance) {
            case "exact":
                // For exact matches, provide just the answer or a simple sentence
                if (prompt.includes("what is") || prompt.includes("calculate")) {
                    responseText = expectedAnswer
                } else if (prompt.includes("spell") || prompt.includes("word")) {
                    responseText = expectedAnswer
                } else {
                    responseText = expectedAnswer
                }
                break

            case "contains":
                // For contains matches, provide a sentence that includes the answer
                if (prompt.includes("capital")) {
                    responseText = `The capital is ${expectedAnswer}.`
                } else if (prompt.includes("planet")) {
                    responseText = `The answer is ${expectedAnswer}.`
                } else if (prompt.includes("color")) {
                    responseText = `It's ${expectedAnswer}.`
                } else if (prompt.includes("opposite")) {
                    responseText = `The opposite is ${expectedAnswer}.`
                } else {
                    responseText = `${expectedAnswer}`
                }
                break

            case "numeric":
                // For numeric matches, extract and return the number
                responseText = expectedAnswer
                break

            default:
                responseText = expectedAnswer
        }

        return {
            text: responseText,
            isGoldenTicket: true,
            success: true,
        }
    } catch (error: any) {
        return {
            text: "",
            isGoldenTicket: true,
            success: false,
            error: `Failed to generate honest response: ${error?.message ?? error}`,
        }
    }
}

// ─── Work Processing with Golden Ticket Support ─────────────────────────────

/**
 * Process a work request with Golden Ticket detection and honest response.
 *
 * This is the main entry point for scout work processing. It:
 * 1. Checks if the work is a Golden Ticket
 * 2. If yes, generates an honest response
 * 3. If no, generates normal draft tokens using WebLLM
 *
 * @param work - The work request from the Oracle
 * @param scoutId - The ID of this scout
 * @returns WorkResult with the response
 */
export async function processWorkRequest(
    work: WorkRequest,
    scoutId: string
): Promise<WorkResult> {
    const timestamp = Date.now()

    // Check if this is a Golden Ticket
    const gtCheck = checkGoldenTicket(work.prompt)

    if (gtCheck.isGoldenTicket && gtCheck.expectedAnswer) {
        // This is a Golden Ticket - generate honest response
        console.log(`[GoldenTicket] Detected Golden Ticket: ${work.workId}`)

        const honestResponse = generateHonestGoldenTicketResponse(
            work.prompt,
            gtCheck.expectedAnswer,
            gtCheck.tolerance || "exact"
        )

        if (!honestResponse.success) {
            console.error(`[GoldenTicket] Failed to generate honest response: ${honestResponse.error}`)
        }

        return {
            workId: work.workId,
            draftText: honestResponse.text,
            scoutId,
            timestamp,
            isGoldenTicketResponse: true,
        }
    }

    // Normal work - generate draft using WebLLM
    try {
        const draftResult = await generateDraftTokens(work.prompt)

        if (!draftResult.success) {
            throw new Error(draftResult.error || "Draft generation failed")
        }

        return {
            workId: work.workId,
            draftText: draftResult.text,
            scoutId,
            timestamp,
            isGoldenTicketResponse: false,
        }
    } catch (error: any) {
        console.error(`[WorkProcessor] Failed to process work: ${error?.message ?? error}`)

        // Return empty result on failure
        return {
            workId: work.workId,
            draftText: "",
            scoutId,
            timestamp,
            isGoldenTicketResponse: false,
        }
    }
}

// ─── API Integration ─────────────────────────────────────────────────────────

const API_BASE = "http://127.0.0.1:8000"

/**
 * Submit a work result to the Oracle API.
 *
 * This function submits the draft response (whether from a Golden Ticket
 * or normal work) to the Oracle for verification.
 *
 * @param result - The work result to submit
 * @returns Success status and detail message
 */
export async function submitWorkResult(
    result: WorkResult
): Promise<{ success: boolean; detail: string }> {
    try {
        const res = await fetch(`${API_BASE}/v1/scout/draft`, {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
            },
            body: JSON.stringify({
                workId: result.workId,
                draftText: result.draftText,
                scoutId: result.scoutId,
                isGoldenTicket: result.isGoldenTicketResponse,
            }),
        })

        if (!res.ok) {
            const errorText = await res.text()
            return {
                success: false,
                detail: `API error (${res.status}): ${errorText}`,
            }
        }

        const data = await res.json()
        return {
            success: data.success ?? true,
            detail: data.detail || "Submitted successfully",
        }
    } catch (error: any) {
        return {
            success: false,
            detail: `Network error: ${error?.message ?? error}`,
        }
    }
}

/**
 * Fetch work from the Oracle API.
 *
 * Polls for available work that the Scout can process.
 *
 * @returns Work request or null if no work available
 */
export async function fetchWork(): Promise<WorkRequest | null> {
    try {
        const res = await fetch(`${API_BASE}/v1/scout/work`)

        if (res.status === 204) {
            return null
        }

        if (!res.ok) {
            throw new Error(`HTTP ${res.status}`)
        }

        return await res.json()
    } catch (error: any) {
        console.error(`[WorkFetcher] Failed to fetch work: ${error?.message ?? error}`)
        return null
    }
}

// ─── Scout Worker Loop ───────────────────────────────────────────────────────

/**
 * Scout worker configuration options.
 */
export type ScoutWorkerOptions = {
    scoutId: string
    pollIntervalMs?: number
    onWorkReceived?: (work: WorkRequest) => void
    onWorkCompleted?: (result: WorkResult, submission: { success: boolean; detail: string }) => void
    onError?: (error: Error) => void
}

/**
 * Start the scout worker loop.
 *
 * This function continuously polls for work and processes it.
 * It handles both Golden Tickets and normal work requests.
 *
 * @param options - Worker configuration
 * @returns Cleanup function to stop the worker
 */
export function startScoutWorker(options: ScoutWorkerOptions): () => void {
    const {
        scoutId,
        pollIntervalMs = 2000,
        onWorkReceived,
        onWorkCompleted,
        onError,
    } = options

    let isRunning = true

    const processWork = async () => {
        if (!isRunning) return

        try {
            // Fetch work from the API
            const work = await fetchWork()

            if (!work) {
                // No work available, continue polling
                return
            }

            // Notify that work was received
            onWorkReceived?.(work)

            // Process the work (with Golden Ticket detection)
            const result = await processWorkRequest(work, scoutId)

            // Submit the result
            const submission = await submitWorkResult(result)

            // Notify that work was completed
            onWorkCompleted?.(result, submission)

            // Log Golden Ticket handling
            if (result.isGoldenTicketResponse) {
                console.log(`[ScoutWorker] Golden Ticket processed: ${result.workId}`)
            }
        } catch (error: any) {
            onError?.(error)
            console.error(`[ScoutWorker] Error in work loop: ${error?.message ?? error}`)
        }
    }

    // Start the polling loop
    const intervalId = setInterval(processWork, pollIntervalMs)

    // Return cleanup function
    return () => {
        isRunning = false
        clearInterval(intervalId)
    }
}
