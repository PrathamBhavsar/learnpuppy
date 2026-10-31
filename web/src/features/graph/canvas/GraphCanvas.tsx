// React shell around GraphEngine: creates it, feeds it props, and forwards its events. All per-frame
// work stays inside the engine; this component re-renders only when props change.
import { useEffect, useRef, useState } from "react"

import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { cn } from "@/lib/utils"
import { useUi } from "@/stores/ui"

import { copy } from "../copy"
import type { Scene } from "../model/scene"

import { GraphEngine, type EngineEvents, type EngineOptions, type Insets } from "./engine"

type CanvasSelection = {
  indices: readonly number[]
  active: number
  /** Nodes that were just picked, for the one-shot ring. */
  pulse: readonly number[]
}

type Props = {
  scene: Scene | null
  visible: Uint8Array | null
  match: Uint8Array | null
  selection: CanvasSelection
  options: Omit<EngineOptions, "reducedMotion">
  insets: Insets
  deleteTint: readonly number[] | null
  handlers: Partial<EngineEvents>
  onEngine: (engine: GraphEngine | null) => void
  className?: string
}

export function GraphCanvas({
  scene,
  visible,
  match,
  selection,
  options,
  insets,
  deleteTint,
  handlers,
  onEngine,
  className,
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const engineRef = useRef<GraphEngine | null>(null)
  const handlersRef = useRef(handlers)
  const reduced = useReducedMotion()
  const resolvedTheme = useUi((s) => s.resolvedTheme)
  // No 2D context (very old or locked-down browsers, jsdom): the list and chain table still work.
  const [unsupported, setUnsupported] = useState(false)

  useEffect(() => {
    handlersRef.current = handlers
  })

  // The engine lives as long as the canvas. StrictMode's double mount creates and destroys one cleanly.
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    let engine: GraphEngine
    try {
      engine = new GraphEngine(canvas)
    } catch {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setUnsupported(true)
      return
    }
    engineRef.current = engine
    const unsubscribe = (
      [
        "hover",
        "click",
        "dblclick",
        "marquee",
        "marqueeRect",
        "camera",
        "births",
      ] as const
    ).map((name) =>
      engine.on(name, ((...args: unknown[]) =>
        (handlersRef.current[name] as ((...a: unknown[]) => void) | undefined)?.(
          ...args
        )) as never)
    )
    const parent = canvas.parentElement
    const measure = () => {
      if (!parent) return
      const rect = parent.getBoundingClientRect()
      engine.resize(rect.width, rect.height, window.devicePixelRatio || 1)
    }
    measure()
    const observer = new ResizeObserver(measure)
    if (parent) observer.observe(parent)
    // Re-read the canvas colors whenever the theme class on <html> flips (also inside a View Transition).
    const theme = new MutationObserver(() => engine.setTheme())
    theme.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] })
    onEngine(engine)
    return () => {
      observer.disconnect()
      theme.disconnect()
      unsubscribe.forEach((off) => off())
      onEngine(null)
      engine.destroy()
      engineRef.current = null
    }
  }, [onEngine])

  useEffect(() => {
    engineRef.current?.setTheme()
  }, [resolvedTheme])

  useEffect(() => {
    engineRef.current?.setInsets(insets)
  }, [insets])

  useEffect(() => {
    engineRef.current?.setOptions({ ...options, reducedMotion: reduced })
  }, [options, reduced])

  // Order matters: a new scene resets the engine's masks, so everything below re-applies after it.
  useEffect(() => {
    engineRef.current?.setScene(scene)
  }, [scene])
  useEffect(() => {
    engineRef.current?.setVisible(visible)
  }, [scene, visible])
  useEffect(() => {
    engineRef.current?.setMatch(match)
  }, [scene, match])
  useEffect(() => {
    engineRef.current?.setSelection(selection.indices, selection.active, selection.pulse)
  }, [scene, selection])
  useEffect(() => {
    engineRef.current?.setDeleteTint(deleteTint)
  }, [scene, deleteTint])

  return (
    <>
      <canvas
        ref={canvasRef}
        data-testid="graph-canvas"
        // Spatial data: x grows to the right even when the surrounding chrome is mirrored.
        dir="ltr"
        aria-hidden
        className={cn("absolute inset-0 size-full touch-none select-none", className)}
      />
      {unsupported ? (
        <p
          data-testid="graph-canvas-unsupported"
          className="absolute inset-x-0 top-1/2 mx-auto max-w-sm -translate-y-1/2 px-4 text-center text-sm text-muted-foreground"
        >
          {copy.canvasUnsupported}
        </p>
      ) : null}
    </>
  )
}
