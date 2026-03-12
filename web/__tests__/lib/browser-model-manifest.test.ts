import { browserModelManifest, getBrowserDraftManifest } from "@/lib/browser-model-manifest"

describe("browser model manifest", () => {
  it("keeps the browser chat and draft model families aligned", () => {
    expect(browserModelManifest.browserChat.primaryModelId).toContain("Llama-3.2-1B")
    expect(browserModelManifest.draft.primaryModelId).toContain("Llama-3.2-1B")
    expect(browserModelManifest.webnnEmbedding.modelPath).toBe("/models/webnn/identity.onnx")
  })

  it("resolves qwen and llama draft presets through the logical manifest", () => {
    expect(getBrowserDraftManifest("qwen").logicalId).toBe("qwen-browser-draft")
    expect(getBrowserDraftManifest("llama").logicalId).toBe("llama-browser-draft")
  })
})
