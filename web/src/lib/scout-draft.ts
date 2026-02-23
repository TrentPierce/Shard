/**
 * Scout Draft Service
 * 
 * Handles draft token generation and submission to the Shard verifier.
 * Part of the speculative decoding loop.
 */

import { apiUrl } from "./config"

// ─── Types ──────────────────────────────────────────────────────────────────

export interface DraftSubmission {
  work_id: string
  scout_id: string
  draft_text: string
  timestamp?: number
}

export interface DraftResponse {
  ok: boolean
  detail?: string
}

export interface ScoutConfig {
  maxDraftTokens: number
  temperature: number
  topP: number
  timeoutMs: number
}

const DEFAULT_CONFIG: ScoutConfig = {
  maxDraftTokens: 4,
  temperature: 0.8,
  topP: 0.9,
  timeoutMs: 800, // Default timeout for verifier response
}

// ─── State ──────────────────────────────────────────────────────────────────

let scoutId: string | null = null
let isSubmitting = false

// ─── Helpers ──────────────────────────────────────────────────────────────

function getScoutId(): string {
  if (!scoutId) {
    // Try to get existing ID from localStorage
    if (typeof window !== "undefined") {
      scoutId = localStorage.getItem("shard_scout_id")
      if (!scoutId) {
        scoutId = `scout_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`
        localStorage.setItem("shard_scout_id", scoutId)
      }
    } else {
      scoutId = `scout_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`
    }
  }
  return scoutId
}

// ─── Draft Submission API ────────────────────────────────────────────────

/**
 * Submit draft tokens to the verifier for speculative decoding.
 * 
 * @param workId - Unique ID for this work item (matches the request)
 * @param draftText - The draft tokens as a space-separated string
 * @param config - Optional configuration overrides
 * @returns Promise with the verifier's response
 */
export async function submitDraft(
  workId: string,
  draftText: string,
  config: Partial<ScoutConfig> = {}
): Promise<DraftResponse> {
  const cfg = { ...DEFAULT_CONFIG, ...config }
  const submission: DraftSubmission = {
    work_id: workId,
    scout_id: getScoutId(),
    draft_text: draftText,
    timestamp: Date.now() / 1000, // Unix timestamp in seconds
  }

  // Create abort controller for timeout
  const controller = new AbortController()
  const timeoutId = setTimeout(() => controller.abort(), cfg.timeoutMs)

  try {
    isSubmitting = true
    
    const response = await fetch(apiUrl("/v1/scout/draft"), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(submission),
      signal: controller.signal,
    })

    clearTimeout(timeoutId)

    if (!response.ok) {
      const error = await response.json().catch(() => ({ detail: "Unknown error" }))
      return {
        ok: false,
        detail: error.detail || `HTTP ${response.status}`,
      }
    }

    const result = await response.json()
    isSubmitting = false
    return result
  } catch (error) {
    clearTimeout(timeoutId)
    isSubmitting = false
    
    if (error instanceof Error) {
      if (error.name === "AbortError") {
        return {
          ok: false,
          detail: `Timeout: verifier did not respond within ${cfg.timeoutMs}ms`,
        }
      }
      return {
        ok: false,
        detail: error.message,
      }
    }
    
    return {
      ok: false,
      detail: "Unknown error submitting draft",
    }
  }
}

/**
 * Check if the scout is currently submitting a draft.
 */
export function isDraftSubmitting(): boolean {
  return isSubmitting
}

/**
 * Cancel any in-progress draft submission.
 * Note: This uses a best-effort approach since the request may already be complete.
 */
export function cancelDraftSubmission(): void {
  isSubmitting = false
  // The actual abort would need to be handled by the caller
  // passing a signal to submitDraft (future enhancement)
}

// ─── Work Polling API ────────────────────────────────────────────────────

export interface WorkItem {
  work: {
    request_id: string
    prompt: string
    max_tokens: number
  } | null
}

/**
 * Poll for available work from the verifier.
 * Called periodically by the scout to check if there's pending work.
 * 
 * @param scoutId - The scout's unique identifier
 * @returns WorkItem if work is available, null otherwise
 */
export async function pollForWork(scoutId: string): Promise<WorkItem> {
  try {
    const response = await fetch(
      apiUrl(`/v1/scout/work?scout_id=${encodeURIComponent(scoutId)}`),
      {
        method: "GET",
        headers: {
          "Content-Type": "application/json",
        },
      }
    )

    if (!response.ok) {
      return { work: null }
    }

    return await response.json()
  } catch {
    return { work: null }
  }
}

// ─── Draft Generation + Submission Combined ───────────────────────────────

export interface DraftResult {
  success: boolean
  workId: string
  draftText: string
  submitted: boolean
  error?: string
}

/**
 * Complete flow: generate draft tokens and submit to verifier.
 * 
 * This combines the WebLLM draft generation with submission to the verifier.
 * Used by the chat flow when in distributed inference mode.
 * 
 * @param prompt - The prompt to generate drafts for
 * @param workId - Unique work identifier
 * @param generateDraftFn - Function to generate draft tokens (from WebLLM)
 * @param config - Optional configuration
 * @returns DraftResult with success status and details
 */
export async function generateAndSubmitDraft(
  prompt: string,
  workId: string,
  generateDraftFn: (prompt: string) => Promise<string[]>,
  config: Partial<ScoutConfig> = {}
): Promise<DraftResult> {
  const cfg = { ...DEFAULT_CONFIG, ...config }

  try {
    // Step 1: Generate draft tokens using WebLLM
    const tokens = await generateDraftFn(prompt)
    
    if (tokens.length === 0) {
      return {
        success: false,
        workId,
        draftText: "",
        submitted: false,
        error: "No tokens generated",
      }
    }

    // Step 2: Join tokens into draft text (space-separated)
    const draftText = tokens.join(" ")

    // Step 3: Submit to verifier
    const response = await submitDraft(workId, draftText, config)

    return {
      success: response.ok,
      workId,
      draftText,
      submitted: response.ok,
      error: response.detail,
    }
  } catch (error) {
    return {
      success: false,
      workId,
      draftText: "",
      submitted: false,
      error: error instanceof Error ? error.message : "Unknown error",
    }
  }
}
