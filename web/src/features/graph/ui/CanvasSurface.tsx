/* eslint-disable jsx-a11y/no-noninteractive-tabindex -- a custom keyboard-driven canvas is exactly what role="application" is for */
import type { ComponentProps } from "react"

/** The focusable region around the canvas: it owns keyboard navigation and the context menu. */
export function CanvasSurface(props: ComponentProps<"div">) {
  return (
    <div
      tabIndex={0}
      role="application"
      aria-roledescription="graph"
      {...props}
    />
  )
}
