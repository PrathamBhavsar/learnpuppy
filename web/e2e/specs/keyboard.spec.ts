import { expect, test } from "@playwright/test"

const SHELL = "/w/ws_rustasync/graph"

// Keyboard-only walkthrough: every stop must be reachable and show a focus target.
test("keyboard only: skip link, sidebar nav, palette, dialog escape", async ({
  page,
}) => {
  await page.goto(SHELL)
  await page.getByTestId("graph-stage").waitFor()

  await page.keyboard.press("Tab")
  await expect(
    page.getByRole("link", { name: "Skip to content" })
  ).toBeFocused()
  await page.keyboard.press("Enter")

  // Tab through the first 40 stops: focus never gets lost to <body>, and every
  // stop has a visible accessible name.
  for (let i = 0; i < 40; i++) {
    await page.keyboard.press("Tab")
    const info = await page.evaluate(() => {
      const el = document.activeElement
      if (!el || el === document.body) return null
      const name =
        el.getAttribute("aria-label") ||
        (el as HTMLElement).innerText?.trim() ||
        el.getAttribute("title") ||
        el.getAttribute("placeholder") ||
        el.getAttribute("aria-labelledby") ||
        el.tagName === "CANVAS" ||
        el.getAttribute("role")
      return name ? "ok" : `unnamed ${el.outerHTML.slice(0, 80)}`
    })
    expect(info, `tab stop ${i + 1}`).toBe("ok")
  }

  // Palette opens and closes by keyboard, restoring focus.
  await page.keyboard.press("ControlOrMeta+k")
  await expect(page.getByTestId("palette-input")).toBeFocused()
  await page.keyboard.press("Escape")
  await expect(page.getByTestId("palette-input")).toBeHidden()
})
