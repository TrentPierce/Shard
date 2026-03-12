export const DEFAULT_EMBEDDING_DIMENSIONS = 96

function tokenize(text: string): string[] {
    return text
        .toLowerCase()
        .replace(/[^a-z0-9\s]+/g, " ")
        .split(/\s+/)
        .map((token) => token.trim())
        .filter((token) => token.length > 1)
}

function hashToken(token: string): number {
    let hash = 2166136261
    for (let idx = 0; idx < token.length; idx += 1) {
        hash ^= token.charCodeAt(idx)
        hash = Math.imul(hash, 16777619)
    }
    return hash >>> 0
}

export function buildHashedTextEmbedding(
    text: string,
    dimensions = DEFAULT_EMBEDDING_DIMENSIONS,
): Float32Array {
    const vector = new Float32Array(dimensions)
    const tokens = tokenize(text)
    if (tokens.length === 0) {
        return vector
    }

    for (const token of tokens) {
        const hash = hashToken(token)
        const index = hash % dimensions
        const sign = (hash & 1) === 0 ? 1 : -1
        const magnitude = 1 + Math.min(token.length, 12) / 12
        vector[index] += sign * magnitude
    }

    let norm = 0
    for (let idx = 0; idx < vector.length; idx += 1) {
        norm += vector[idx] * vector[idx]
    }
    if (norm <= 0) {
        return vector
    }
    const scale = 1 / Math.sqrt(norm)
    for (let idx = 0; idx < vector.length; idx += 1) {
        vector[idx] *= scale
    }
    return vector
}

export function cosineSimilarity(
    left: ArrayLike<number>,
    right: ArrayLike<number>,
): number {
    const length = Math.min(left.length, right.length)
    if (length === 0) {
        return 0
    }

    let dot = 0
    let leftNorm = 0
    let rightNorm = 0
    for (let idx = 0; idx < length; idx += 1) {
        const a = Number(left[idx] ?? 0)
        const b = Number(right[idx] ?? 0)
        dot += a * b
        leftNorm += a * a
        rightNorm += b * b
    }
    if (leftNorm <= 0 || rightNorm <= 0) {
        return 0
    }
    return dot / Math.sqrt(leftNorm * rightNorm)
}
