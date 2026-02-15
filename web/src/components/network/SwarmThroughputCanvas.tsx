"use client"

import { useEffect, useRef } from "react"
import type { ThroughputSample } from "@/lib/mockSwarmTelemetry"

type SwarmThroughputCanvasProps = {
  samples: ThroughputSample[]
}

export default function SwarmThroughputCanvas({ samples }: SwarmThroughputCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null)

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    const context = canvas.getContext("2d")
    if (!context) return

    const { width, height } = canvas
    context.clearRect(0, 0, width, height)

    const values = samples.map((sample) => sample.tflops)
    const min = Math.min(...values) - 4
    const max = Math.max(...values) + 4
    const range = max - min || 1

    context.fillStyle = "rgba(8, 13, 32, 0.95)"
    context.fillRect(0, 0, width, height)

    context.strokeStyle = "rgba(0, 212, 255, 0.15)"
    context.lineWidth = 1
    for (let i = 1; i < 4; i += 1) {
      const y = (height / 4) * i
      context.beginPath()
      context.moveTo(0, y)
      context.lineTo(width, y)
      context.stroke()
    }

    const points = values.map((value, index) => {
      const x = (index / (values.length - 1 || 1)) * (width - 30) + 15
      const y = height - ((value - min) / range) * (height - 28) - 14
      return { x, y }
    })

    const gradient = context.createLinearGradient(0, 0, width, height)
    gradient.addColorStop(0, "rgba(0, 212, 255, 0.95)")
    gradient.addColorStop(1, "rgba(139, 92, 246, 0.95)")

    context.strokeStyle = gradient
    context.lineWidth = 2.2
    context.beginPath()
    points.forEach((point, idx) => {
      if (idx === 0) {
        context.moveTo(point.x, point.y)
      } else {
        context.lineTo(point.x, point.y)
      }
    })
    context.stroke()

    const areaGradient = context.createLinearGradient(0, 0, 0, height)
    areaGradient.addColorStop(0, "rgba(0, 212, 255, 0.28)")
    areaGradient.addColorStop(1, "rgba(0, 212, 255, 0)")

    context.fillStyle = areaGradient
    context.beginPath()
    points.forEach((point, idx) => {
      if (idx === 0) {
        context.moveTo(point.x, point.y)
      } else {
        context.lineTo(point.x, point.y)
      }
    })
    context.lineTo(points[points.length - 1].x, height - 12)
    context.lineTo(points[0].x, height - 12)
    context.closePath()
    context.fill()

    const lastPoint = points[points.length - 1]
    context.beginPath()
    context.fillStyle = "#00d4ff"
    context.arc(lastPoint.x, lastPoint.y, 4, 0, Math.PI * 2)
    context.fill()

    context.font = "11px JetBrains Mono, monospace"
    context.fillStyle = "rgba(179, 189, 227, 0.95)"
    context.fillText(`${samples[samples.length - 1]?.timestamp ?? ""} UTC`, 14, height - 12)
    context.fillText(`${max.toFixed(1)} TFLOPs peak`, width - 118, 14)
  }, [samples])

  return <canvas ref={canvasRef} width={780} height={250} style={{ width: "100%", height: "250px", borderRadius: "14px" }} />
}
