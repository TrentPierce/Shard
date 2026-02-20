/// <reference types="@webgpu/types" />
/**
 * WebGPU Memory Sandbox Guard
 *
 * Enforces memory limits on WebGPU device allocations to prevent out-of-memory
 * crashes in the browser tab. Wraps standard GPUDevice methods with size limits.
 */

export interface GpuSandboxConfig {
    maxAllocationMb?: number
    maxTotalMb?: number
}

const DEFAULT_CONFIG: Required<GpuSandboxConfig> = {
    maxAllocationMb: 256,
    maxTotalMb: 1024,
}

export class GpuSandboxGuard {
    private device: GPUDevice
    private config: Required<GpuSandboxConfig>
    private totalAllocatedBytes = 0

    constructor(device: GPUDevice, config?: GpuSandboxConfig) {
        this.device = device
        this.config = { ...DEFAULT_CONFIG, ...config }

        // Catch device lost errors early
        this.device.lost.then((info) => {
            console.warn(`[GpuSandbox] GPUDevice lost: ${info.reason} - ${info.message}`)
        })
    }

    get rawDevice(): GPUDevice {
        return this.device
    }

    /**
     * Safe wrapper around device.createBuffer.
     */
    createBuffer(descriptor: GPUBufferDescriptor): GPUBuffer {
        const sizeBytes = descriptor.size

        if (sizeBytes > this.config.maxAllocationMb * 1024 * 1024) {
            throw new Error(`[GpuSandbox] Buffer allocation of ${sizeBytes} bytes exceeds individual limit of ${this.config.maxAllocationMb}MB`)
        }

        if (this.totalAllocatedBytes + sizeBytes > this.config.maxTotalMb * 1024 * 1024) {
            throw new Error(`[GpuSandbox] Buffer allocation would exceed total limit of ${this.config.maxTotalMb}MB`)
        }

        const buffer = this.device.createBuffer(descriptor)
        this.totalAllocatedBytes += sizeBytes

        // Optional: could patch buffer.destroy to decrement totalAllocatedBytes,
        // but WebLLM handles its own memory and we generally just cap the peak.
        return buffer
    }

    /**
     * Destroy the device and clean up.
     */
    destroy() {
        this.device.destroy()
        this.totalAllocatedBytes = 0
    }
}

/**
 * Helper to request an adapter and device, then wrap it in the sandbox.
 */
export async function requestSandboxedDevice(
    config?: GpuSandboxConfig
): Promise<{ adapter: GPUAdapter; device: GPUDevice; sandbox: GpuSandboxGuard }> {
    if (!navigator.gpu) {
        throw new Error('WebGPU is not supported in this browser.')
    }

    const adapter = await navigator.gpu.requestAdapter({
        powerPreference: 'high-performance',
    })

    if (!adapter) {
        throw new Error('No appropriate GPU adapter found.')
    }

    // Request standard WebLLM required limits, but capped by our sandbox later
    const requiredLimits: Record<string, number> = {}
    if (adapter.limits.maxBufferSize) {
        requiredLimits.maxBufferSize = adapter.limits.maxBufferSize
    }
    if (adapter.limits.maxComputeWorkgroupStorageSize) {
        requiredLimits.maxComputeWorkgroupStorageSize = adapter.limits.maxComputeWorkgroupStorageSize
    }
    if (adapter.limits.maxComputeInvocationsPerWorkgroup) {
        requiredLimits.maxComputeInvocationsPerWorkgroup = adapter.limits.maxComputeInvocationsPerWorkgroup
    }

    const device = await adapter.requestDevice({
        requiredLimits,
    })

    const sandbox = new GpuSandboxGuard(device, config)

    // Create a proxy to intercept createBuffer calls without breaking WebLLM's 
    // expected GPUDevice interface.
    const proxyDevice = new Proxy(device, {
        get: (target, prop, receiver) => {
            if (prop === 'createBuffer') {
                return sandbox.createBuffer.bind(sandbox)
            }
            if (prop === 'destroy') {
                return sandbox.destroy.bind(sandbox)
            }
            const value = Reflect.get(target, prop, receiver)
            return typeof value === 'function' ? value.bind(target) : value
        }
    })

    return { adapter, device: proxyDevice, sandbox }
}
