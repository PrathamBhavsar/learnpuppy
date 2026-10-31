import type { Page } from "@playwright/test"

export const GRAPH = "/w/ws_rustasync/graph"

/** The subset of window.__lp.graph these specs use (src/features/graph/test-hook.ts). */
type GraphHook = {
  nodeCount: () => number
  selection: () => string[]
  active: () => string | null
  hover: () => string | null
  camera: () => { x: number; y: number; k: number; z: number }
  screenPosition: (id: string) => { x: number; y: number } | null
  frameStats: () => {
    frames: number
    costP95: number
    gapP95: number
    gapMax: number
  }
  resetFrameStats: () => void
  loadFixture: (nodes: number) => Promise<void>
}
export type LpWindow = { __lp?: { graph?: GraphHook } }

export async function openGraph(page: Page, path = GRAPH) {
  const errors: string[] = []
  page.on("pageerror", (e) => errors.push(e.message))
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()))
  await page.goto(path)
  await page.waitForFunction(
    () => (window as LpWindow).__lp?.graph?.nodeCount() ?? 0 > 0
  )
  await settle(page)
  return errors
}

/** Waits for the load-in and any camera fly to finish (the camera stops moving). */
export async function settle(page: Page) {
  let last = ""
  for (let i = 0; i < 40; i++) {
    const now = JSON.stringify(await cam(page))
    if (now === last) return
    last = now
    await page.waitForTimeout(120)
  }
}

export const cam = (page: Page) =>
  page.evaluate(() => (window as LpWindow).__lp?.graph?.camera())
export const selection = (page: Page) =>
  page.evaluate(() => (window as LpWindow).__lp?.graph?.selection() ?? [])
export const count = (page: Page) =>
  page.evaluate(() => (window as LpWindow).__lp?.graph?.nodeCount() ?? 0)

/** Most connected propositions first; read from the accessible list (ids are `graph-list-<id>`). */
export async function listedIds(page: Page, n = 12): Promise<string[]> {
  const ids = await page
    .getByTestId("graph-list-item")
    .evaluateAll((els) => els.map((el) => el.id.replace(/^graph-list-/, "")))
  return ids.slice(0, n)
}

/** Screen position of a node, or null when it is off-screen or under another control. */
async function pointOf(page: Page, id: string) {
  return page.evaluate((nodeId) => {
    const p = (window as LpWindow).__lp?.graph?.screenPosition(nodeId)
    if (!p) return null
    const top = document.elementFromPoint(p.x, p.y)
    return top?.closest("[data-testid='graph-canvas']") ? p : null
  }, id)
}

/** Picks `n` listed nodes whose centres are clear of overlays and at least 24px apart. */
export async function clearNodes(page: Page, n: number) {
  const found: { id: string; x: number; y: number }[] = []
  for (const id of await listedIds(page, 40)) {
    const p = await pointOf(page, id)
    if (!p) continue
    if (found.some((f) => Math.hypot(f.x - p.x, f.y - p.y) < 24)) continue
    found.push({ id, ...p })
    if (found.length === n) return found
  }
  throw new Error(`only ${found.length} clear nodes found`)
}
