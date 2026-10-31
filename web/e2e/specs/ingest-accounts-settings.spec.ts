import { mkdirSync } from "node:fs"
import AxeBuilder from "@axe-core/playwright"
import { type Page, expect, test } from "@playwright/test"

const WS = "ws_rustasync"

async function open(page: Page, path: string, testId: string) {
  const errors: string[] = []
  page.on("pageerror", (e) => errors.push(e.message))
  await page.goto(path)
  await expect(page.getByTestId(testId)).toBeVisible()
  return errors
}

test.describe("ingest", () => {
  test("save sources, run with live progress, then read the finished report", async ({
    page,
  }) => {
    const errors = await open(page, `/w/${WS}/ingest`, "ingest-sources")
    await expect(page.getByTestId("allow-item-0")).toBeVisible()

    // detect projects -> add one -> save
    await page.getByTestId("detect-projects").click()
    await page
      .getByTestId("detect-dialog")
      .getByRole("checkbox", { name: /raft-lab/ })
      .click()
    await page.getByTestId("detect-add").click()
    await expect(page.getByTestId("allow-list")).toContainText("raft-lab")
    await page.getByTestId("sources-save").click()
    await expect(page.getByTestId("sources-save")).toBeDisabled()

    // run
    await page.getByTestId("run-start").click()
    await expect(page.getByTestId("run-progress")).toBeVisible()
    await expect(page.getByTestId("run-phase")).toBeVisible()
    await expect(page.getByTestId("last-run")).toBeVisible({ timeout: 8000 })
    await expect(page.getByTestId("run-progress")).toHaveCount(0)

    // errors in an older run are expandable and actionable
    await page.getByTestId("run-toggle-run_2").click()
    await expect(page.getByTestId("run-details-run_2")).toContainText(
      "Close Antigravity"
    )
    expect(errors).toEqual([])
  })

  test("cancel stops a running ingest", async ({ page }) => {
    await open(page, `/w/${WS}/ingest`, "ingest-sources")
    await page.getByTestId("run-start").click()
    await page.getByTestId("run-cancel").click()
    await expect(page.getByTestId("run-progress")).toHaveCount(0)
    await expect(page.getByTestId("run-start")).toBeEnabled()
  })

  test("an empty allow-list explains itself and disables Run", async ({
    page,
  }) => {
    await open(page, `/w/${WS}/ingest`, "ingest-sources")
    for (const index of [1, 0])
      await page.getByTestId(`allow-remove-${index}`).click()
    await expect(page.getByTestId("allow-empty")).toBeVisible()
    await expect(page.getByTestId("run-start")).toBeDisabled()
  })
})

test.describe("accounts", () => {
  test("cooldown ring counts down, disable works, add profile validates", async ({
    page,
  }) => {
    const errors = await open(page, "/accounts", "accounts-list")
    const ring = page.getByTestId("account-ring-claude:acc-2-text")
    await expect(ring).toHaveText(/\d+s/)
    const first = await ring.textContent()
    await expect.poll(async () => ring.textContent()).not.toBe(first)

    const toggle = page.getByTestId("account-toggle-codex:acc-1")
    await toggle.click()
    await expect(page.getByTestId("account-status-codex:acc-1")).toHaveText(
      "Disabled"
    )

    await page.getByTestId("accounts-add").click()
    await page.getByTestId("add-profile-path").fill("/tmp/nope")
    await page.getByTestId("add-profile-submit").click()
    await expect(page.getByTestId("add-profile-error")).toContainText(
      ".claude.json"
    )
    await page.keyboard.press("Escape")
    expect(errors).toEqual([])
  })

  test("role slots: edit, diff preview, save and test ping", async ({
    page,
  }) => {
    await open(page, "/accounts", "global-role-slots")
    await page.getByTestId("global-model-teacher").click()
    await page.getByRole("option", { name: "claude-sonnet-5-5" }).click()
    await page.getByTestId("global-review").click()
    await expect(page.getByTestId("global-diff-list")).toContainText(
      "claude-opus-5-5"
    )
    await page.getByTestId("global-confirm").click()
    await expect(page.getByTestId("global-dirty")).toHaveCount(0)

    await page.getByTestId("global-test-teacher").click()
    await expect(page.getByTestId("global-ping-teacher")).toContainText("OK")
  })
})

test.describe("settings", () => {
  test("theme, direction, motion and density apply at once and survive a reload", async ({
    page,
  }) => {
    await open(page, "/settings", "settings-appearance")
    const html = page.locator("html")

    await page.getByTestId("theme-dark").click()
    await expect(html).toHaveClass(/dark/)
    await page.getByTestId("direction-rtl").click()
    await expect(html).toHaveAttribute("dir", "rtl")
    await page.getByTestId("reduce-motion").click()
    await expect(html).toHaveAttribute("data-reduce-motion", "true")
    await page.getByTestId("density-compact").click()
    await expect(html).toHaveAttribute("data-density", "compact")

    await page.reload()
    await expect(page.getByTestId("settings-appearance")).toBeVisible()
    await expect(html).toHaveClass(/dark/)
    await expect(html).toHaveAttribute("dir", "rtl")
    await expect(html).toHaveAttribute("data-reduce-motion", "true")
    await expect(html).toHaveAttribute("data-density", "compact")
  })

  test("reduced motion leaves no animation running longer than 150 ms", async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: "reduce" })
    await open(page, "/accounts", "accounts-list")
    await page.waitForTimeout(500)
    const long = await page.evaluate(() =>
      document
        .getAnimations()
        .filter((a) => {
          const t = a.effect?.getComputedTiming()
          return (
            t !== undefined &&
            (t.iterations === Infinity ||
              (typeof t.duration === "number" && t.duration > 150))
          )
        })
        .map((a) => a.id || a.constructor.name)
    )
    expect(long).toEqual([])
  })

  test("concurrency cap and diagnostics", async ({ page }) => {
    await open(page, "/settings", "settings-behavior")
    await page.getByRole("slider", { name: "Concurrency cap" }).focus()
    await page.keyboard.press("ArrowRight")
    await expect(page.getByTestId("concurrency-value")).toHaveText("3")
    // The mock db lives in the page, so read the saved document back through the API instead of reloading.
    await expect
      .poll(() =>
        page.evaluate(async () => {
          const res = await fetch("/api/v1/settings")
          return ((await res.json()) as { concurrencyCap: number })
            .concurrencyCap
        })
      )
      .toBe(3)

    await page.getByTestId("copy-diagnostics").click()
    await expect(page.getByTestId("diagnostics-text")).toContainText(
      "LearnPuppy diagnostics"
    )
  })

  test("workspace settings: overrides inherit until switched on", async ({
    page,
  }) => {
    await open(page, `/w/${WS}/settings`, "ws-general")
    await expect(page.getByTestId("ws-role-teacher")).toHaveAttribute(
      "data-locked",
      "true"
    )
    await page.getByTestId("ws-override-teacher").click()
    await expect(page.getByTestId("ws-role-teacher")).toHaveAttribute(
      "data-locked",
      "false"
    )
  })
})

for (const [name, path, testId] of [
  ["ingest", `/w/${WS}/ingest`, "ingest-sources"],
  ["accounts", "/accounts", "accounts-list"],
  ["settings", "/settings", "settings-appearance"],
] as const) {
  test(`axe: ${name} has no serious or critical violations`, async ({
    page,
  }) => {
    await open(page, path, testId)
    await page.waitForTimeout(400)
    const { violations } = await new AxeBuilder({ page }).analyze()
    expect(
      violations
        .filter((v) => v.impact === "serious" || v.impact === "critical")
        .map((v) => `${v.id}: ${v.nodes[0]?.target.join(" ")}`)
    ).toEqual([])
  })
}

// Reference shots: LP_SHOTS=1 bunx playwright test ingest-accounts-settings -g shot
const SHOTS = [
  ["ingest", `/w/${WS}/ingest`, "ingest-sources"],
  ["accounts", "/accounts", "accounts-list"],
  ["settings", "/settings", "settings-appearance"],
] as const

for (const [name, path, testId] of SHOTS) {
  for (const [theme, dir] of [
    ["dark", "ltr"],
    ["light", "ltr"],
    ["dark", "rtl"],
  ] as const) {
    test(`shot ${name}-${theme}-${dir}`, async ({ page }) => {
      test.skip(!process.env["LP_SHOTS"], "set LP_SHOTS=1 for reference shots")
      mkdirSync("docs-screenshots", { recursive: true })
      await page.setViewportSize({ width: 1280, height: 1000 })
      await page.emulateMedia({ reducedMotion: "reduce" })
      await page.addInitScript(
        (t) => localStorage.setItem("lp-theme", t),
        theme
      )
      await page.goto(`${path}?dir=${dir}`)
      await expect(page.getByTestId(testId)).toBeVisible()
      await page.waitForTimeout(500)
      await page.screenshot({
        path: `docs-screenshots/${name}-${theme}-${dir}.png`,
      })
    })
  }
}
