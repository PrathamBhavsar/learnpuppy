import { createFileRoute } from "@tanstack/react-router"
import { lazy, Suspense } from "react"

import { SkeletonRows } from "@/components/app/skeleton-block"
import { validateGraphSearch } from "@/features/graph/use-graph-url"

// The canvas, d3 modules and chain table are a heavy chunk; keep them out of the shell bundle.
const GraphScreen = lazy(() => import("@/features/graph/GraphScreen"))

function Screen() {
  const { ws } = Route.useParams()
  return (
    <Suspense
      fallback={
        <div className="p-6" aria-busy>
          <SkeletonRows rows={6} className="max-w-3xl" />
        </div>
      }
    >
      <GraphScreen ws={ws} />
    </Suspense>
  )
}

export const Route = createFileRoute("/_shell/w/$ws/graph")({
  validateSearch: validateGraphSearch,
  component: Screen,
})
