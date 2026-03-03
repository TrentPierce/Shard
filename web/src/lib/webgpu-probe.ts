export interface WebGPUProbeResult {
  eligible: boolean
  reason?: string
  tier: "high-performance" | "low-power" | "none"
  estimated_vram_mb: number
  supports_f16: boolean
  browser: string
  os: string
  adapter_vendor: string
  adapter_device: string
}

const TWO_GB = 2 * 1024 * 1024 * 1024

function detectBrowser(userAgent: string): string {
  if (/Edg\//i.test(userAgent)) return "Edge"
  if (/Chrome\//i.test(userAgent) && !/Edg\//i.test(userAgent)) return "Chrome"
  if (/Safari\//i.test(userAgent) && !/Chrome\//i.test(userAgent)) return "Safari"
  if (/Firefox\//i.test(userAgent)) return "Firefox"
  return "Unknown"
}

function detectOs(userAgent: string, platform?: string): string {
  const source = `${platform ?? ""} ${userAgent}`.toLowerCase()
  if (source.includes("win")) return "Windows"
  if (source.includes("mac")) return "macOS"
  if (source.includes("android")) return "Android"
  if (source.includes("iphone") || source.includes("ipad") || source.includes("ios")) return "iOS"
  if (source.includes("linux")) return "Linux"
  return "Unknown"
}

function ineligible(reason: string, browser: string, os: string): WebGPUProbeResult {
  return {
    eligible: false,
    reason,
    tier: "none",
    estimated_vram_mb: 0,
    supports_f16: false,
    browser,
    os,
    adapter_vendor: "unknown",
    adapter_device: "unknown",
  }
}

export async function probeWebGPU(): Promise<WebGPUProbeResult> {
  const ua = typeof navigator !== "undefined" ? navigator.userAgent ?? "" : ""
  const browser = detectBrowser(ua)
  const os = detectOs(ua, typeof navigator !== "undefined" ? navigator.platform : undefined)

  if (typeof navigator === "undefined" || !("gpu" in navigator)) {
    return ineligible("no_navigator_gpu", browser, os)
  }

  let tier: "high-performance" | "low-power" = "high-performance"
  let adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" })
  if (!adapter) {
    tier = "low-power"
    adapter = await navigator.gpu.requestAdapter({ powerPreference: "low-power" })
  }
  if (!adapter) {
    return ineligible("no_adapter", browser, os)
  }

  const adapterWithInfo = adapter as GPUAdapter & {
    requestAdapterInfo?: () => Promise<{ vendor?: string; device?: string }>
  }

  let vendor = "unknown"
  let deviceName = "unknown"
  if (typeof adapterWithInfo.requestAdapterInfo === "function") {
    try {
      const info = await adapterWithInfo.requestAdapterInfo()
      vendor = info.vendor ?? "unknown"
      deviceName = info.device ?? "unknown"
    } catch {
      // Ignore adapter info lookup failure.
    }
  }

  const supportsF16 = adapter.features.has("shader-f16")
  const estimatedVramMb = Math.floor((adapter.limits.maxBufferSize ?? 0) / (1024 * 1024))

  let gpuDevice: GPUDevice | null = null
  try {
    gpuDevice = await adapter.requestDevice()
  } catch {
    return ineligible("request_device_failed", browser, os)
  }

  if (gpuDevice.limits.maxStorageBufferBindingSize < TWO_GB) {
    gpuDevice.destroy()
    return {
      eligible: false,
      reason: "insufficient_vram",
      tier,
      estimated_vram_mb: estimatedVramMb,
      supports_f16: supportsF16,
      browser,
      os,
      adapter_vendor: vendor,
      adapter_device: deviceName,
    }
  }

  gpuDevice.destroy()
  return {
    eligible: true,
    tier,
    estimated_vram_mb: estimatedVramMb,
    supports_f16: supportsF16,
    browser,
    os,
    adapter_vendor: vendor,
    adapter_device: deviceName,
  }
}
