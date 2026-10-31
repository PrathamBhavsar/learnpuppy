import { useCallback, useEffect, useRef, useState } from "react"

export type Transform = { k: number; x: number; y: number }

const MIN_K = 0.3
const MAX_K = 3

/** Transform that centres `content` inside `view` at the largest scale <= 1 that fits. */
export function fitTransform(
  content: { width: number; height: number },
  view: { width: number; height: number }
): Transform {
  if (view.width === 0 || view.height === 0) return { k: 1, x: 0, y: 0 }
  const k = Math.min(
    1,
    view.width / content.width,
    view.height / content.height
  )
  return {
    k,
    x: (view.width - content.width * k) / 2,
    y: (view.height - content.height * k) / 2,
  }
}

/**
 * Wheel zoom (around the pointer) and drag pan for an SVG viewport. Until the user interacts the
 * view follows `fit`; `reset` hands control back to it.
 */
export function usePanZoom(fit: Transform) {
  const ref = useRef<SVGSVGElement>(null)
  const [manual, setManual] = useState<Transform | null>(null)
  const drag = useRef<{ x: number; y: number; origin: Transform } | null>(null)
  const current = manual ?? fit
  const currentRef = useRef(current)
  useEffect(() => {
    currentRef.current = current
  })

  const zoomAt = useCallback((factor: number, cx: number, cy: number) => {
    const t = currentRef.current
    const k = Math.min(MAX_K, Math.max(MIN_K, t.k * factor))
    const ratio = k / t.k
    setManual({ k, x: cx - (cx - t.x) * ratio, y: cy - (cy - t.y) * ratio })
  }, [])

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const onWheel = (event: WheelEvent) => {
      event.preventDefault()
      const rect = el.getBoundingClientRect()
      zoomAt(
        Math.exp(-event.deltaY * 0.0015),
        event.clientX - rect.left,
        event.clientY - rect.top
      )
    }
    el.addEventListener("wheel", onWheel, { passive: false })
    return () => el.removeEventListener("wheel", onWheel)
  }, [zoomAt])

  const handlers = {
    onPointerDown: (event: React.PointerEvent<SVGSVGElement>) => {
      if (event.button !== 0 || event.target !== event.currentTarget) return
      event.currentTarget.setPointerCapture(event.pointerId)
      drag.current = {
        x: event.clientX,
        y: event.clientY,
        origin: currentRef.current,
      }
    },
    onPointerMove: (event: React.PointerEvent<SVGSVGElement>) => {
      const d = drag.current
      if (!d) return
      setManual({
        ...d.origin,
        x: d.origin.x + event.clientX - d.x,
        y: d.origin.y + event.clientY - d.y,
      })
    },
    onPointerUp: () => {
      drag.current = null
    },
  }

  return {
    ref,
    transform: current,
    handlers,
    zoomBy: (factor: number, view: { width: number; height: number }) =>
      zoomAt(factor, view.width / 2, view.height / 2),
    reset: () => setManual(null),
    moved: manual !== null,
  }
}
