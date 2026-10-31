// `window.__lp.graph` for Playwright and component tests (docs/web/07 §Browser test hooks). Installed
// only in test and mock builds; the production bundle drops it because MODE is a build-time constant.
import type { Cam } from "./canvas/camera"
import type { GraphEngine } from "./canvas/engine"

export type GraphTestState = {
  nodeCount: () => number
  selection: () => string[]
  active: () => string | null
  hover: () => string | null
  camera: () => Cam & { z: number }
  /** Node ids currently drawn on screen with their pixel positions. */
  screenPosition: (id: string) => { x: number; y: number } | null
  frameStats: () => ReturnType<GraphEngine["stats"]>
  resetFrameStats: () => void
  hullAlpha: () => number
  /** Mock mode only: swap this workspace's graph for a generated one of `nodes` nodes (perf work). */
  loadFixture: (nodes: number) => Promise<void>
}

const testHooksEnabled = (): boolean =>
  import.meta.env.MODE === "test" || import.meta.env.MODE === "mock"

// `Window.__lp` is typed by the mock bootstrap for the SSE hook alone; the graph adds its own key.
type LpWindow = { __lp?: Record<string, unknown> }

/** Registers the graph hook; returns a disposer. A no-op outside test builds. */
export function installGraphTestHook(hook: GraphTestState): () => void {
  if (!testHooksEnabled()) return () => {}
  const w = window as unknown as LpWindow
  w.__lp = { ...(w.__lp ?? {}), graph: hook }
  return () => {
    const lp = (window as unknown as LpWindow).__lp
    if (lp && lp["graph"] === hook) delete lp["graph"]
  }
}

/** Replaces the mock database's graph for `ws` with a generated large one. Mock builds only. */
export async function loadMockGraph(ws: string, nodes: number): Promise<void> {
  if (import.meta.env.MODE !== "mock") return
  const [{ state }, { buildLargeGraph }] = await Promise.all([
    import("@/test/msw/db"),
    import("@/test/msw/fixtures/graph-large"),
  ])
  state.db.graphs.set(ws, buildLargeGraph(ws, nodes, state.db.now, 99))
}
