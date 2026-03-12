"use client"

import { useEffect, useState } from "react"
import {
    detectScoutCapability,
    type ScoutCapabilityResult,
    getCapabilityLabel,
    getCapabilityReason,
    getCapabilityRecommendation,
    type ScoutCapability,
} from "@/lib/scout-capability"
import { AlertTriangle, CheckCircle, XCircle, Cpu } from "lucide-react"

interface ScoutCapabilityBadgeProps {
    showDetails?: boolean
    size?: "sm" | "md" | "lg"
}

export default function ScoutCapabilityBadge({
    showDetails = true,
    size = "md",
}: ScoutCapabilityBadgeProps) {
    const [capability, setCapability] = useState<ScoutCapabilityResult | null>(null)
    const [loading, setLoading] = useState(true)

    useEffect(() => {
        detectScoutCapability().then((result) => {
            setCapability(result)
            setLoading(false)
        })
    }, [])

    if (loading) {
        return (
            <div className={`scout-capability-badge loading ${size}`}>
                <Cpu className="animate-pulse" size={iconSize[size]} />
                <span>Detecting...</span>
            </div>
        )
    }

    if (!capability) {
        return null
    }

    const config = badgeConfig[capability.capability]

    return (
        <div className={`scout-capability-badge ${capability.capability} ${size}`}>
            <div className="capability-icon">
                {config.icon}
            </div>
            <div className="capability-info">
                <span className="capability-label">
                    {getCapabilityLabel(capability.capability)}
                </span>
                {showDetails && (
                    <>
                        <span className="capability-reason">
                            {getCapabilityReason(capability)}
                        </span>
                        {getCapabilityRecommendation(capability) && (
                            <span className="capability-recommendation">
                                {getCapabilityRecommendation(capability)}
                            </span>
                        )}
                    </>
                )}
            </div>
        </div>
    )
}

const iconSize = {
    sm: 14,
    md: 18,
    lg: 24,
}

const badgeConfig: Record<
    ScoutCapability,
    {
        icon: React.ReactNode
        bgClass: string
        textClass: string
    }
> = {
    webgpu: {
        icon: <CheckCircle size={18} />,
        bgClass: "bg-green-500/10",
        textClass: "text-green-500",
    },
    wasm: {
        icon: <AlertTriangle size={18} />,
        bgClass: "bg-yellow-500/10",
        textClass: "text-yellow-500",
    },
    unsupported: {
        icon: <XCircle size={18} />,
        bgClass: "bg-red-500/10",
        textClass: "text-red-500",
    },
}
