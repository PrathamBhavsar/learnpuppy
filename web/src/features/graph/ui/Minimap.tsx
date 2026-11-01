// Overview of the whole layout with the current viewport outlined; drag to move the view.
import { useCallback, useEffect, useRef } from "react"

import { readCanvasTheme } from "../canvas/theme"
import { viewBounds } from "../canvas/camera"
import type { GraphEngine } from "../canvas/engine"
import { copy } from "../copy"
import type { Scene } from "../model/scene"

const W = 176
const H = 116
const PAD = 8
const MAX_DOTS = 3000

type Props = {
  engine: GraphEngine | null
  scene: Scene | null
  visible: Uint8Array | null
}

export function Minimap({ engine, scene, visible }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const layer = useRef<HTMLCanvasElement | null>(null)
  const frame = useRef(0)
  const map = useRef({ scale: 1, ox: 0, oy: 0 })

  // The node dots only change with the scene, so they are painted once into an offscreen layer.
  const paintLayer = useCallback(() => {
    if (!scene) return
    const dpr = window.devicePixelRatio || 1
    const off = document.createElement("canvas")
    off.width = W * dpr
    off.height = H * dpr
    const ctx = off.getContext("2d")
    if (!ctx) return
    ctx.scale(dpr, dpr)
    const { bounds } = scene
    const bw = Math.max(bounds.maxX - bounds.minX, 1)
    const bh = Math.max(bounds.maxY - bounds.minY, 1)
    const scale = Math.min((W - PAD * 2) / bw, (H - PAD * 2) / bh)
    map.current = {
      scale,
      ox: (W - bw * scale) / 2 - bounds.minX * scale,
      oy: (H - bh * scale) / 2 - bounds.minY * scale,
    }
    const theme = readCanvasTheme()
    const step = Math.max(1, Math.ceil(scene.count / MAX_DOTS))
    ctx.globalAlpha = 0.75
    for (let i = 0; i < scene.count; i += step) {
      if (visible && !visible[i]) continue
      ctx.fillStyle =
        theme.sources[scene.source[i] as number] ?? theme.foreground
      ctx.fillRect(
        (scene.x[i] as number) * scale + map.current.ox - 1,
        (scene.y[i] as number) * scale + map.current.oy - 1,
        2,
        2
      )
    }
    layer.current = off
  }, [scene, visible])

  const draw = useCallback(() => {
    frame.current = 0
    const canvas = canvasRef.current
    const ctx = canvas?.getContext("2d")
    if (!canvas || !ctx || !engine) return
    const dpr = window.devicePixelRatio || 1
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, W, H)
    if (layer.current) ctx.drawImage(layer.current, 0, 0, W, H)
    const theme = readCanvasTheme()
    const { scale, ox, oy } = map.current
    const v = viewBounds(engine.camera.cam, engine.size)
    ctx.strokeStyle = theme.primary
    ctx.fillStyle = theme.primary
    const x = v.minX * scale + ox
    const y = v.minY * scale + oy
    const w = (v.maxX - v.minX) * scale
    const h = (v.maxY - v.minY) * scale
    ctx.globalAlpha = 0.1
    ctx.fillRect(x, y, w, h)
    ctx.globalAlpha = 1
    ctx.lineWidth = 1.25
    ctx.strokeRect(x + 0.5, y + 0.5, Math.max(w - 1, 1), Math.max(h - 1, 1))
  }, [engine])

  const schedule = useCallback(() => {
    if (!frame.current) frame.current = requestAnimationFrame(draw)
  }, [draw])

  useEffect(() => {
    paintLayer()
    schedule()
  }, [paintLayer, schedule])

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const dpr = window.devicePixelRatio || 1
    canvas.width = W * dpr
    canvas.height = H * dpr
    schedule()
  }, [schedule])

  useEffect(() => {
    if (!engine) return
    const off = engine.on("camera", schedule)
    return () => {
      off()
      cancelAnimationFrame(frame.current)
      frame.current = 0
    }
  }, [engine, schedule])

  const moveTo = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!engine) return
    const rect = e.currentTarget.getBoundingClientRect()
    const { scale, ox, oy } = map.current
    const wx = (e.clientX - rect.left - ox) / scale
    const wy = (e.clientY - rect.top - oy) / scale
    const cam = engine.camera.cam
    engine.camera.jumpTo({
      k: cam.k,
      x: engine.size.w / 2 - wx * cam.k,
      y: engine.size.h / 2 - wy * cam.k,
    })
  }

  return (
    <canvas
      ref={canvasRef}
      role="img"
      aria-label={copy.minimapLabel}
      data-testid="graph-minimap"
      dir="ltr"
      style={{ width: W, height: H }}
      className="pointer-events-auto cursor-pointer touch-none border border-border bg-background/90"
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId)
        moveTo(e)
      }}
      onPointerMove={(e) => {
        if (e.currentTarget.hasPointerCapture(e.pointerId)) moveTo(e)
      }}
    />
  )
}
