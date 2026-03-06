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

    if (values.length === 0) {
      context.fillStyle = "rgba(17, 24, 39, 0.95)"
      context.fillRect(0, 0, width, height)
      context.font = "12px IBM Plex Mono, monospace"
      context.fillStyle = "rgba(148, 163, 184, 0.82)"
      context.fillText("Waiting for telemetry samples...", 14, height / 2)
      return
    }

    const min = Math.min(...values) - 4
    const max = Math.max(...values) + 4
    const range = max - min || 1

    context.fillStyle = "rgba(17, 24, 39, 0.95)"
    context.fillRect(0, 0, width, height)

    context.strokeStyle = "rgba(148, 163, 184, 0.18)"
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

    context.strokeStyle = "rgba(118, 146, 255, 0.95)"
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

    context.fillStyle = "rgba(118, 146, 255, 0.18)"
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
    context.fillStyle = "#7692ff"
    context.arc(lastPoint.x, lastPoint.y, 4, 0, Math.PI * 2)
    context.fill()

    context.font = "11px IBM Plex Mono, monospace"
    context.fillStyle = "rgba(148, 163, 184, 0.95)"
    context.fillText(`${samples[samples.length - 1]?.timestamp ?? ""} UTC`, 14, height - 12)
    context.fillText(`${max.toFixed(1)} TFLOPs peak`, width - 118, 14)
  }, [samples])

  return <canvas ref={canvasRef} width={780} height={250} style={{ width: "100%", height: "250px", borderRadius: "14px" }} />
}

