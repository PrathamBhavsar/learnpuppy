import AxeBuilder from "@axe-core/playwright"
import { type Page, expect, test } from "@playwright/test"

const LEARN = "/w/ws_rustasync/learn"

async function collectErrors(page: Page) {
  const errors: string[] = []
  page.on("pageerror", (e) => errors.push(e.message))
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()))
  return errors
}

async function seriousViolations(page: Page) {
  const { violations } = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze()
  return violations
    .filter((v) => v.impact === "serious" || v.impact === "critical")
    .map((v) => `${v.id}: ${v.nodes[0]?.target.join(" ")}`)
}

test("learn lists chats, opens one, and shows its plan and pending quiz", async ({
  page,
}) => {
  const errors = await collectErrors(page)
  await page.goto(LEARN)
  await expect(page.getByTestId("start-learning")).toBeVisible()
  await expect(page.getByTestId("chat-item-c_teach_quiz")).toBeVisible()
  await page.getByTestId("chat-item-c_teach_quiz").getByRole("link").click()
  await expect(page).toHaveURL(/\/learn\/c_teach_quiz$/)
  await expect(page.getByTestId("classroom-title")).toHaveText(
    "Futures, polling and wakers"
  )
  await expect(page.getByTestId("question-dock")).toHaveAttribute(
    "data-tool",
    "quiz"
  )
  await expect(page.getByTestId("plan-node-p2")).toHaveAttribute(
    "data-status",
    "current"
  )
  await expect(page.getByTestId("phase-teach")).toHaveAttribute(
    "data-state",
    "active"
  )
  expect(errors).toEqual([])
})

test("answering a quiz shows the server's verdict in the conversation", async ({
  page,
}) => {
  await page.goto(`${LEARN}/c_teach_quiz`)
  await page.getByTestId("quiz-option-A").click()
  await page.getByTestId("quiz-submit").click()
  await expect(page.getByTestId("verdict-call_quiz_2")).toBeVisible()
  await expect(page.getByTestId("question-dock")).toBeHidden()
})

test("quiz options answer to keys and Enter", async ({ page }) => {
  await page.goto(`${LEARN}/c_teach_quiz`)
  await page.getByTestId("question-dock").waitFor()
  await page.keyboard.press("b")
  await expect(page.getByTestId("quiz-option-B")).toHaveAttribute(
    "aria-checked",
    "true"
  )
  await page.keyboard.press("Enter")
  await expect(page.getByTestId("verdict-call_quiz_2")).toBeVisible()
})

test("a new chat streams the tutor's reply and settles it", async ({
  page,
}) => {
  await page.goto(LEARN)
  await page.getByTestId("start-topic").fill("How do wakers work?")
  await page.getByTestId("start-submit").click()
  await expect(page).toHaveURL(/\/learn\/c_\d+$/)
  await expect(page.getByTestId("streaming-text")).toBeVisible()
  await expect(
    page.getByTestId("message-assistant").filter({ hasText: "waker" })
  ).toBeVisible({ timeout: 10_000 })
  await expect(page.getByTestId("streaming-text")).toBeHidden()
})

test("jobs open their artifacts, HTML inside an inert iframe", async ({
  page,
}) => {
  await page.goto(`${LEARN}/c_teach_quiz`)
  await page.getByTestId("tab-jobs").click()
  await page.getByTestId("job-open-job_visual_done").click()
  const frame = page.getByTestId("artifact-html")
  await expect(frame).toBeVisible()
  await expect(frame).toHaveAttribute("sandbox", "")
  await expect(
    page.frameLocator("[data-testid=artifact-html]").locator("h3")
  ).toHaveText("Poll loop")
})

test("rate-limit banner counts down; engine errors offer Retry", async ({
  page,
}) => {
  await page.goto(`${LEARN}/c_queued`)
  await expect(page.getByTestId("rate-limit-banner")).toContainText(
    "Waiting for an available model account"
  )
  const first = await page.getByTestId("rate-limit-countdown").innerText()
  await expect
    .poll(async () => page.getByTestId("rate-limit-countdown").innerText())
    .not.toBe(first)
  await page.goto(`${LEARN}/c_error`)
  await expect(page.getByTestId("turn-error")).toContainText("No model account")
  await expect(page.getByTestId("turn-retry")).toBeVisible()
})

test("the plan panel resizes with the keyboard and remembers its width", async ({
  page,
}) => {
  await page.goto(`${LEARN}/c_teach_quiz`)
  const handle = page.getByTestId("plan-resize")
  await handle.focus()
  await page.keyboard.press("ArrowLeft")
  await expect(handle).toHaveAttribute("aria-valuenow", "356")
  await page.reload()
  await expect(page.getByTestId("plan-resize")).toHaveAttribute(
    "aria-valuenow",
    "356"
  )
})

test("rename and delete go through dialogs", async ({ page }) => {
  await page.goto(LEARN)
  await page.getByTestId("chat-menu-c_done").click()
  await page.getByRole("menuitem", { name: /Rename/ }).click()
  await page.getByTestId("rename-input").fill("Backpressure")
  await page.getByTestId("rename-submit").click()
  await expect(page.getByTestId("chat-item-c_done")).toContainText(
    "Backpressure"
  )
  await page.getByTestId("chat-menu-c_done").click()
  await page.getByRole("menuitem", { name: /Delete/ }).click()
  await expect(page.getByTestId("delete-dialog")).toContainText("chat trash")
  await page.getByTestId("delete-confirm").click()
  await expect(page.getByTestId("chat-item-c_done")).toHaveCount(0)
})

for (const theme of ["dark", "light"] as const) {
  for (const dir of ["ltr", "rtl"] as const) {
    test(`classroom is axe-clean (${theme}, ${dir})`, async ({ page }) => {
      await page.addInitScript(
        (t) => localStorage.setItem("lp-theme", t),
        theme
      )
      await page.goto(`${LEARN}/c_teach_quiz?dir=${dir}`)
      await page.getByTestId("question-dock").waitFor()
      await page.getByTestId("plan-dag").waitFor()
      await page.waitForTimeout(700)
      expect(await seriousViolations(page)).toEqual([])
      await page.getByTestId("tab-jobs").click()
      expect(await seriousViolations(page)).toEqual([])
    })
  }
}

test("RTL puts the plan panel at the inline-end (left) edge, plan DAG stays LTR", async ({
  page,
}) => {
  await page.goto(`${LEARN}/c_teach_quiz?dir=rtl`)
  const plan = await page.getByTestId("plan-panel").boundingBox()
  const conversation = await page.getByTestId("message-list").boundingBox()
  expect(plan && conversation && plan.x < conversation.x).toBe(true)
  await expect(page.getByTestId("plan-dag")).toHaveAttribute("dir", "ltr")
})

test("reduced motion leaves no running animation longer than 150 ms", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" })
  await page.goto(`${LEARN}/c_teach_quiz`)
  await page.getByTestId("plan-dag").waitFor()
  await page.waitForTimeout(500)
  const long = await page.evaluate(() =>
    document
      .getAnimations()
      .filter((a) => a.playState === "running")
      .map((a) => Number(a.effect?.getComputedTiming().activeDuration ?? 0))
      .filter((d) => d > 150)
  )
  expect(long).toEqual([])
})

test("an unknown chat explains itself", async ({ page }) => {
  await page.goto(`${LEARN}/nope`)
  await expect(page.getByText("Chat not found")).toBeVisible()
})
