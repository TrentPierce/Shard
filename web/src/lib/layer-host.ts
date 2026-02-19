import { rustUrl } from "./config"
import { xorStreamTransformHex } from "./activation-obfuscation"

type BrowserLayerRegisterResponse = {
  ok: boolean
  session_id: string
  model_id: string
  layer_start: number
  layer_end: number
  expires_at_ms: number
  obfuscation_key_hex: string
}

type BrowserLayerWorkItem = {
  work_id: string
  session_id: string
  request_id: string
  step_id: string
  source_peer_id: string
  tensor_name: string
  dtype: number
  shape: number[]
  nonce_hex: string
  obfuscated_tensor_hex: string
  created_at_ms: number
}

type BrowserLayerWorkResponse = {
  ok: boolean
  status?: string
  work?: BrowserLayerWorkItem
}

let hostStopper: (() => void) | null = null

async function profileWebGPU() {
  const gpu = (navigator as any).gpu
  const adapter = await gpu?.requestAdapter()
  if (!adapter) {
    throw new Error("WebGPU adapter not available")
  }
  const vendor = (adapter as any).info?.vendor as string | undefined
  return {
    supports_webgpu: true,
    webgpu_vendor: vendor ?? "unknown",
    max_buffer_size_mb: Math.floor(Number(adapter.limits.maxBufferSize) / (1024 * 1024)),
    max_storage_buffers_per_stage: Number(adapter.limits.maxStorageBuffersPerShaderStage),
    user_agent: navigator.userAgent,
  }
}

async function gpuPassThrough(inputBytes: Uint8Array): Promise<Uint8Array> {
  const gpu = (navigator as any).gpu
  if (!gpu) {
    return inputBytes
  }
  const adapter = await gpu.requestAdapter()
  const device = await adapter?.requestDevice()
  if (!device) {
    return inputBytes
  }

  const MAP_READ = 0x0001
  const COPY_SRC = 0x0004
  const COPY_DST = 0x0008
  const size = Math.max(4, inputBytes.byteLength)
  const src = device.createBuffer({
    size,
    usage: COPY_SRC | COPY_DST,
    mappedAtCreation: true,
  })
  new Uint8Array(src.getMappedRange()).set(inputBytes)
  src.unmap()

  const dst = device.createBuffer({
    size,
    usage: COPY_SRC | COPY_DST,
  })
  const readback = device.createBuffer({
    size,
    usage: COPY_DST | MAP_READ,
  })

  const encoder = device.createCommandEncoder()
  encoder.copyBufferToBuffer(src, 0, dst, 0, size)
  encoder.copyBufferToBuffer(dst, 0, readback, 0, size)
  device.queue.submit([encoder.finish()])

  await readback.mapAsync(MAP_READ)
  const out = new Uint8Array(readback.getMappedRange()).slice(0, inputBytes.byteLength)
  readback.unmap()
  src.destroy()
  dst.destroy()
  readback.destroy()
  return out
}

function hexToBytes(hex: string): Uint8Array {
  const out = new Uint8Array(hex.length / 2)
  for (let i = 0; i < out.length; i += 1) {
    out[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16)
  }
  return out
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes).map((v) => v.toString(16).padStart(2, "0")).join("")
}

async function processWork(
  work: BrowserLayerWorkItem,
  obfuscationKeyHex: string
): Promise<void> {
  const plaintextHex = await xorStreamTransformHex(
    obfuscationKeyHex,
    work.nonce_hex,
    work.obfuscated_tensor_hex
  )
  const gpuBytes = await gpuPassThrough(hexToBytes(plaintextHex))
  const outputHex = bytesToHex(gpuBytes)
  const obfuscatedOutHex = await xorStreamTransformHex(
    obfuscationKeyHex,
    work.nonce_hex,
    outputHex
  )

  await fetch(rustUrl("/browser-layer/submit"), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      session_id: work.session_id,
      work_id: work.work_id,
      request_id: work.request_id,
      step_id: work.step_id,
      source_peer_id: work.source_peer_id,
      tensor_name: work.tensor_name,
      dtype: work.dtype,
      shape: work.shape,
      nonce_hex: work.nonce_hex,
      obfuscated_tensor_hex: obfuscatedOutHex,
    }),
  })
}

export async function startBrowserLayerHost(options?: {
  modelId?: string
  layerStart?: number
  layerEnd?: number
}): Promise<() => void> {
  if (hostStopper) {
    return hostStopper
  }
  if (!(navigator as any).gpu) {
    throw new Error("WebGPU unavailable")
  }

  const profile = await profileWebGPU()
  const register = await fetch(rustUrl("/browser-layer/register"), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      model_id: options?.modelId ?? "default-model",
      layer_start: options?.layerStart ?? 0,
      layer_end: options?.layerEnd ?? 1,
      profile,
    }),
  })
  if (!register.ok) {
    throw new Error(`browser layer registration failed (${register.status})`)
  }
  const session = (await register.json()) as BrowserLayerRegisterResponse
  if (!session.ok) {
    throw new Error("browser layer registration rejected")
  }

  let stopped = false
  const loop = async () => {
    while (!stopped) {
      try {
        const res = await fetch(
          rustUrl(`/browser-layer/work?session_id=${encodeURIComponent(session.session_id)}`)
        )
        if (!res.ok) {
          await new Promise((resolve) => setTimeout(resolve, 500))
          continue
        }
        const payload = (await res.json()) as BrowserLayerWorkResponse
        if (payload.ok && payload.work) {
          await processWork(payload.work, session.obfuscation_key_hex)
          continue
        }
      } catch {
      }
      await new Promise((resolve) => setTimeout(resolve, 250))
    }
  }
  void loop()

  hostStopper = () => {
    stopped = true
    hostStopper = null
  }
  return hostStopper
}
