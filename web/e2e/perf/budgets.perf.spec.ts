import { type CDPSession, expect, test } from "@playwright/test"

const WS = "ws_rustasync"
type MockWindow = { __lpMock?: { dropSse: () => void; restoreSse: () => void } }

async function heapMb(cdp: CDPSession) {
  await cdp.send("HeapProfiler.collectGarbage")
  await cdp.send("HeapProfiler.collectGarbage")
  const { usedSize } = (await cdp.send("Runtime.getHeapUsage")) as {
    usedSize: number
  }
  return usedSize / 1024 / 1024
}

test("LCP on a cold load <= 1.2 s (dev server, localhost)", async ({
  page,
}) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __lcp: number }).__lcp = 0
    new PerformanceObserver((l) => {
      for (const e of l.getEntries())
        (window as unknown as { __lcp: number }).__lcp = e.startTime
    }).observe({ type: "largest-contentful-paint", buffered: true })
  })
  await page.goto(`/w/${WS}/review`)
  await expect(page.getByTestId("nav-review-badge")).toBeVisible()
  await page.waitForTimeout(500)
  const lcp = await page.evaluate(
    () => (window as unknown as { __lcp: number }).__lcp
  )
  console.log(`LCP ${lcp.toFixed(0)} ms`)
  expect(lcp).toBeLessThanOrEqual(1200)
})

test("route switches: no long task over 100 ms (cached chunks)", async ({
  page,
}) => {
  await page.goto(`/w/${WS}/review`)
  await expect(page.getByTestId("nav-review-badge")).toBeVisible()
  // Warm every route chunk once.
  for (const r of ["graph", "learn", "digest", "ingest", "review"]) {
    await page.goto(`/w/${WS}/${r}`)
    await page.waitForTimeout(300)
  }
  await page.evaluate(() => {
    const w = window as unknown as { __long: number[] }
    w.__long = []
    new PerformanceObserver((l) => {
      for (const e of l.getEntries()) w.__long.push(e.duration)
    }).observe({ type: "longtask", buffered: false })
  })
  const times: number[] = []
  for (const key of ["g l", "g r", "g d", "g r"]) {
    const t0 = Date.now()
    for (const k of key.split(" ")) await page.keyboard.press(k)
    await page.waitForTimeout(250)
    times.push(Date.now() - t0)
  }
  const long = await page.evaluate(
    () => (window as unknown as { __long: number[] }).__long
  )
  console.log(`long tasks during route switches: ${JSON.stringify(long)}`)
  expect(long.filter((d) => d > 100)).toEqual([])
})

test("memory: heap growth < 30 MB across 200 route switches and 20 SSE reconnects", async ({
  page,
}) => {
  test.setTimeout(240_000)
  const cdp = await page.context().newCDPSession(page)
  await page.goto(`/w/${WS}/review`)
  await expect(page.getByTestId("workspace-chip")).toHaveAccessibleName(
    "Workspace: Rust async"
  )
  // In-app SPA switching (no reloads) is what leaks, so drive chords instead of goto.
  const spa = async (n: number) => {
    for (let i = 0; i < n; i++) {
      await page.keyboard.press("g")
      await page.keyboard.press(["l", "r", "d"][i % 3] ?? "r")
      if (i % 10 === 0) {
        await page.evaluate(() => (window as MockWindow).__lpMock?.dropSse())
        await page.waitForTimeout(50)
        await page.evaluate(() => (window as MockWindow).__lpMock?.restoreSse())
      }
    }
  }
  await spa(30) // warm up caches
  const before = await heapMb(cdp)
  await spa(200)
  const after = await heapMb(cdp)
  console.log(`heap ${before.toFixed(1)} MB -> ${after.toFixed(1)} MB`)
  expect(after - before).toBeLessThan(30)
})
