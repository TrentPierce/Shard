const NONCE_BYTES = 12

function hexToBytes(hex: string): Uint8Array {
  if (!hex || hex.length % 2 !== 0) {
    throw new Error("invalid hex payload")
  }
  const out = new Uint8Array(hex.length / 2)
  for (let i = 0; i < out.length; i += 1) {
    out[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16)
  }
  return out
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("")
}

async function sha256(data: Uint8Array): Promise<Uint8Array> {
  const view = data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength) as ArrayBuffer
  const digest = await crypto.subtle.digest("SHA-256", view)
  return new Uint8Array(digest)
}

function concatBytes(parts: Uint8Array[]): Uint8Array {
  const length = parts.reduce((total, part) => total + part.length, 0)
  const out = new Uint8Array(length)
  let offset = 0
  for (const part of parts) {
    out.set(part, offset)
    offset += part.length
  }
  return out
}

export async function xorStreamTransformHex(
  keyHex: string,
  nonceHex: string,
  payloadHex: string
): Promise<string> {
  const key = hexToBytes(keyHex)
  const nonce = hexToBytes(nonceHex)
  if (nonce.length !== NONCE_BYTES) {
    throw new Error("invalid nonce length")
  }
  const input = hexToBytes(payloadHex)
  if (key.length === 0) {
    return payloadHex
  }

  const out = new Uint8Array(input.length)
  let offset = 0
  let counter = 0

  while (offset < input.length) {
    const counterBytes = new Uint8Array(4)
    new DataView(counterBytes.buffer).setUint32(0, counter, true)
    const block = await sha256(concatBytes([key, nonce, counterBytes]))
    const take = Math.min(block.length, input.length - offset)
    for (let i = 0; i < take; i += 1) {
      out[offset + i] = input[offset + i] ^ block[i]
    }
    offset += take
    counter += 1
  }

  return bytesToHex(out)
}
