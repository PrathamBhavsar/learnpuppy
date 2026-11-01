import AxeBuilder from "@axe-core/playwright"
import { expect, test, type Page } from "@playwright/test"
import { mkdir } from "node:fs/promises"
const LEARN = "/w/ws_rustasync/learn"
const evidence = "../scratchpad/k-b2"
async function fixture(page: Page, kind: string) {
  await page.evaluate(async (kind) => {
    const moduleUrl = "/src/test/msw/db.ts"
    const { state } = await import(moduleUrl)
    const id = kind === "tall" ? "c_teach_quiz" : "c_done"
    const chat = state.db.chats
      .get("ws_rustasync")
      .find((c: { id: string }) => c.id === id)
    if (kind === "tall") {
      chat.pending.prompt =
        "Choose [0, 1) and evaluate \\(x^2\\). " +
        "Explain every step. ".repeat(15)
      chat.pending.options = Array.from({ length: 12 }, (_, i) => ({
        key: `opt-${i}`,
        text:
          `Option ${i + 1}: $x^2$ in [0, 1). ` +
          "A long explanation. ".repeat(12),
      }))
    } else {
      chat.messages = [
        {
          id: "render-fixture",
          role: "assistant",
          calls: [],
          text: 'Inline $x^2$, display $$\\int_0^1 x dx$$ and \\(a+b\\). Interval [0, 1) and literal ](x). Inline `x[0]`.\n\n```c\nint main() {\n    return 0;\n}\n```\n\n```rust\nfn main() {\n    println!("hello");\n}\n```\n\n```python\ndef square(x):\n    return x * x\n```',
        },
      ]
    }
    chat.viewVersion += 1
    ;(
      window as Window & {
        __lpMock?: {
          emit: (type: string, data: unknown, workspace: string) => void
        }
      }
    ).__lpMock?.emit(
      "chat.rest",
      {
        chatId: id,
        viewVersion: chat.viewVersion,
      },
      "ws_rustasync"
    )
  }, kind)
}
async function axe(page: Page) {
  const { violations } = await new AxeBuilder({ page }).analyze()
  expect(
    violations
      .filter((v) => v.impact === "serious" || v.impact === "critical")
      .map((v) => v.id)
  ).toEqual([])
}
test("draft survives screens, chats and reload", async ({ page }) => {
  await page.goto(`${LEARN}/c_done`)
  const input = page.getByTestId("composer-input")
  await input.fill("unfinished explanation")
  await page.goto("/w/ws_rustasync/review")
  await page.goto(`${LEARN}/c_practice_done`)
  await expect(input).toHaveValue("")
  await page.goto(`${LEARN}/c_done`)
  await expect(input).toHaveValue("unfinished explanation")
  await page.reload()
  await expect(input).toHaveValue("unfinished explanation")
})
for (const size of [
  { width: 1280, height: 720 },
  { width: 800, height: 600 },
]) {
  test(`tall question at ${size.width}x${size.height}`, async ({ page }) => {
    await page.setViewportSize(size)
    await page.goto(`${LEARN}/c_teach_quiz`)
    await page.getByTestId("question-dock").waitFor()
    await page.getByTestId("plan-toggle").click()
    await fixture(page, "tall")
    const dock = page.getByTestId("question-dock")
    await expect(dock).toContainText("Option 12")
    const geometry = await dock.evaluate((el) => ({
      height: el.clientHeight,
      scroll: el.scrollHeight,
    }))
    expect(geometry.scroll).toBeGreaterThan(geometry.height)
    expect(geometry.height).toBeLessThanOrEqual(size.height * 0.56)
    await expect(page.getByTestId("quiz-submit")).toBeVisible()
    await page.getByTestId("quiz-option-A").click()
    await mkdir(evidence, { recursive: true })
    await page.screenshot({
      path: `${evidence}/tall-question-${size.width}x${size.height}.png`,
    })
    await axe(page)
  })
}
test("800px drawer traps focus and Escape restores it; shortcut opens", async ({
  page,
}) => {
  await page.setViewportSize({ width: 800, height: 600 })
  await page.goto("/w/ws_rustasync/review")
  const trigger = page.getByTestId("sidebar-drawer-toggle")
  await trigger.click()
  const drawer = page.getByRole("dialog", { name: "Navigation" })
  await expect(drawer).toBeVisible()
  for (let i = 0; i < 18; i++) await page.keyboard.press("Tab")
  expect(
    await drawer.evaluate((el) => el.contains(document.activeElement))
  ).toBe(true)
  await mkdir(evidence, { recursive: true })
  await page.screenshot({ path: `${evidence}/sidebar-800px.png` })
  await page.keyboard.press("Escape")
  await expect(drawer).toBeHidden()
  await expect(trigger).toBeFocused()
  await page.keyboard.press("ControlOrMeta+b")
  await expect(drawer).toBeVisible()
  // Let the open animation finish: axe mid-fade measures a half-transparent surface.
  await drawer.evaluate((el) =>
    Promise.all(el.getAnimations({ subtree: true }).map((a) => a.finished))
  )
  await axe(page)
})
test("math intervals and indented code render", async ({ page }) => {
  await page.goto(`${LEARN}/c_done`)
  await page.getByTestId("classroom").waitFor()
  await page.getByTestId("plan-toggle").click()
  await fixture(page, "render")
  const message = page.locator('[data-message-id="render-fixture"]')
  await expect(message.locator(".katex")).toHaveCount(3)
  await expect(message).toContainText("Interval [0, 1) and literal ](x)")
  await expect(message.locator("a")).toHaveCount(0)
  await expect(message.getByTestId("code-block")).toHaveCount(3)
  expect(await message.locator("pre").last().textContent()).toContain(
    "    return x * x"
  )
  await mkdir(evidence, { recursive: true })
  await page.screenshot({
    path: `${evidence}/math-intervals-code.png`,
    fullPage: true,
  })
})
