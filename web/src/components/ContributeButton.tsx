/**
 * Contribute Button Component
 * 
 * Allows users to start contributing to the network directly from the web UI.
 * Provides platform-specific download links and status tracking.
 */

"use client"

import { useState } from "react"

interface ContributeButtonProps {
  className?: string
}

type ContributionTier = "browser" | "light" | "full" | "oracle"

interface TierInfo {
  id: ContributionTier
  name: string
  description: string
  requirements: string
  benefits: string
}

const TIERS: TierInfo[] = [
  {
    id: "browser",
    name: "Browser Scout",
    description: "Use your browser to generate draft tokens",
    requirements: "WebGPU-capable browser (Chrome/Edge/Brave)",
    benefits: "Free access to API"
  },
  {
    id: "light",
    name: "Light Node",
    description: "Run a lightweight daemon with minimal GPU requirements",
    requirements: "2GB VRAM, 4GB RAM",
    benefits: "Priority access, higher limits"
  },
  {
    id: "full",
    name: "Full Node",
    description: "Run a full Oracle node with full model inference",
    requirements: "8GB+ VRAM, 16GB+ RAM",
    benefits: "Unlimited access, earn reputation"
  },
  {
    id: "oracle",
    name: "Oracle Server",
    description: "Run a public Oracle node serving API requests",
    requirements: "16GB+ VRAM, dedicated server or data center",
    benefits: "Earn priority tokens, network revenue share"
  }
]

export function ContributeButton({ className = "" }: ContributeButtonProps) {
  const [showModal, setShowModal] = useState(false)
  const [selectedTier, setSelectedTier] = useState<ContributionTier | null>(null)
  const [downloadStatus, setDownloadStatus] = useState<string>("idle")

  const getDownloadUrl = (tier: ContributionTier) => {
    const baseUrl = "https://github.com/TrentPierce/Shard/releases"
    switch (tier) {
      case "browser":
        return "#webgpu" // Opens WebLLM modal
      case "light":
        return `${baseUrl}/download/v0.4.0/shard-0.4.0-windows-x64.exe`
      case "full":
        return `${baseUrl}/download/v0.4.0/shard-0.4.0-windows-x64.exe`
      case "oracle":
        return `${baseUrl}/download/v0.4.0/shard-0.4.0-linux-x86_64.AppImage`
      default:
        return baseUrl
    }
  }

  const handleDownload = async (tier: ContributionTier) => {
    setDownloadStatus("downloading")
    
    // For browser tier, open WebLLM modal
    if (tier === "browser") {
      setDownloadStatus("browser")
      // Trigger WebLLM initialization
      return
    }
    
    // For other tiers, initiate download
    const url = getDownloadUrl(tier)
    window.open(url, "_blank")
    setDownloadStatus("downloaded")
  }

  return (
    <>
      <button
        onClick={() => setShowModal(true)}
        className={`px-4 py-2 bg-gradient-to-r from-emerald-500 to-cyan-500 text-white font-semibold rounded-lg hover:opacity-90 transition-opacity ${className}`}
      >
        Contribute
      </button>

      {showModal && (
        <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-4">
          <div className="bg-slate-900 rounded-xl max-w-2xl w-full max-h-[80vh] overflow-y-auto">
            <div className="p-6">
              <div className="flex justify-between items-center mb-6">
                <h2 className="text-2xl font-bold text-white">
                  Contribute to Shard
                </h2>
                <button
                  onClick={() => setShowModal(false)}
                  className="text-slate-400 hover:text-white"
                >
                  ✕
                </button>
              </div>

              <p className="text-slate-300 mb-6">
                Choose how you want to contribute. Every contribution helps
                make AI more accessible to everyone.
              </p>

              <div className="space-y-4">
                {TIERS.map((tier) => (
                  <button
                    key={tier.id}
                    onClick={() => setSelectedTier(tier.id)}
                    className={`w-full p-4 rounded-lg border-2 text-left transition-all ${
                      selectedTier === tier.id
                        ? "border-emerald-500 bg-emerald-500/10"
                        : "border-slate-700 hover:border-slate-600"
                    }`}
                  >
                    <div className="flex justify-between items-start">
                      <div>
                        <h3 className="text-lg font-semibold text-white">
                          {tier.name}
                        </h3>
                        <p className="text-slate-400 text-sm mt-1">
                          {tier.description}
                        </p>
                      </div>
                      {selectedTier === tier.id && (
                        <span className="text-emerald-500">✓</span>
                      )}
                    </div>
                  </button>
                ))}
              </div>

              {selectedTier && (
                <div className="mt-6 p-4 bg-slate-800 rounded-lg">
                  <p className="text-sm text-slate-400 mb-2">
                    <strong className="text-white">Requirements:</strong>{" "}
                    {TIERS.find((t) => t.id === selectedTier)?.requirements}
                  </p>
                  <p className="text-sm text-slate-400">
                    <strong className="text-white">Benefits:</strong>{" "}
                    {TIERS.find((t) => t.id === selectedTier)?.benefits}
                  </p>
                </div>
              )}

              <div className="mt-6 flex gap-3">
                <button
                  onClick={() => handleDownload(selectedTier!)}
                  disabled={!selectedTier}
                  className="flex-1 py-3 bg-emerald-600 text-white font-semibold rounded-lg hover:bg-emerald-500 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  {downloadStatus === "downloading"
                    ? "Downloading..."
                    : downloadStatus === "downloaded"
                    ? "Download Started!"
                    : "Download"}
                </button>
                <button
                  onClick={() => setShowModal(false)}
                  className="px-6 py-3 border border-slate-600 text-slate-300 rounded-lg hover:bg-slate-800 transition-colors"
                >
                  Cancel
                </button>
              </div>

              <p className="text-xs text-slate-500 mt-4 text-center">
                By contributing, you agree to the{" "}
                <a href="/terms" className="underline hover:text-slate-400">
                  Terms of Service
                </a>
              </p>
            </div>
          </div>
        </div>
      )}
    </>
  )
}
