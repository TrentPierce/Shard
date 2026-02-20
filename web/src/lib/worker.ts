/// <reference types="@webgpu/types" />
import { WebWorkerMLCEngineHandler } from "@mlc-ai/web-llm"
import { requestSandboxedDevice } from "./gpu_sandbox"

// Intercept WebGPU initialization to inject our memory sandbox
if (typeof navigator !== "undefined" && navigator.gpu) {
    const originalRequestAdapter = navigator.gpu.requestAdapter.bind(navigator.gpu)
    navigator.gpu.requestAdapter = async function (options?: GPURequestAdapterOptions) {
        // Find adapter normally first
        const adapter = await originalRequestAdapter(options)
        if (!adapter) return null

        // Intercept its requestDevice method
        const originalRequestDevice = adapter.requestDevice.bind(adapter)
        adapter.requestDevice = async function (descriptor?: GPUDeviceDescriptor) {
            // First get the real device
            const device = await originalRequestDevice(descriptor)

            // Wrap it in our sandbox
            const { GpuSandboxGuard } = await import("./gpu_sandbox")
            const sandbox = new GpuSandboxGuard(device)

            // Return Proxy
            return new Proxy(device, {
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
        }

        return adapter
    }
}

const handler = new WebWorkerMLCEngineHandler()

self.onmessage = (msg: MessageEvent) => {
    handler.onmessage(msg)
}
