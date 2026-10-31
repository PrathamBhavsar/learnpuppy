import AxeBuilder from "@axe-core/playwright"
import { expect, test } from "@playwright/test"

for (const dir of ["ltr", "rtl"] as const) {
  test(`reviews five cards with only the keyboard (${dir})`, async ({
    page,
  }) => {
    await page.addInitScript(
      (direction) => localStorage.setItem("lp-dir", direction),
      dir
    )
    await page.goto(
      "/w/ws_rustasync/review?decisions=dec_2,dec_3,dec_4,dec_5,dec_6"
    )
    const start = page.getByTestId("review-start")
    await expect(start).toHaveText("Start review (5)")
    // Reach Start from the initial page focus using Tab; never click or programmatically focus.
    for (
      let i = 0;
      i < 35 && !(await start.evaluate((el) => el === document.activeElement));
      i++
    ) {
      await page.keyboard.press("Tab")
    }
    await expect(start).toBeFocused()
    await page.keyboard.press("Enter")
    for (let i = 0; i < 5; i++) {
      await expect(page.getByTestId("review-progress-label")).toHaveText(
        `Card ${i + 1} of 5`
      )
      await page.keyboard.press(i % 2 ? "b" : "2")
      await expect(
        page
          .locator('[data-testid="review-card"][data-active="true"]')
          .getByTestId("review-rating-bar")
      ).toBeVisible()
      await page.keyboard.press("Space")
    }
    await expect(page.getByTestId("review-summary")).toContainText(
      "5 cards reviewed"
    )
    await expect(page.getByTestId("review-summary-exit")).toBeEnabled()
    await expect
      .poll(() =>
        page
          .getByTestId("review-summary")
          .evaluate((el) => getComputedStyle(el).opacity)
      )
      .toBe("1")
    const violations = (
      await new AxeBuilder({ page }).analyze()
    ).violations.filter(
      (v) => v.impact === "serious" || v.impact === "critical"
    )
    expect(
      violations.map(
        (v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`
      )
    ).toEqual([])
  })
}

for (const theme of ["dark", "light"] as const) {
  for (const dir of ["ltr", "rtl"] as const) {
    test(`review and digest are accessible (${theme}, ${dir})`, async ({
      page,
    }) => {
      await page.addInitScript(
        ([t, d]) => {
          localStorage.setItem("lp-theme", t!)
          localStorage.setItem("lp-dir", d!)
        },
        [theme, dir]
      )
      await page.emulateMedia({ reducedMotion: "reduce" })
      for (const path of ["review", "digest"]) {
        await page.goto(`/w/ws_rustasync/${path}`)
        await expect(
          page.getByTestId(path === "review" ? "review-start" : "digest-days")
        ).toBeVisible()
        const violations = (
          await new AxeBuilder({ page }).analyze()
        ).violations.filter(
          (v) => v.impact === "serious" || v.impact === "critical"
        )
        expect(
          violations.map((v) => `${v.id}: ${v.nodes[0]?.target.join(" ")}`)
        ).toEqual([])
        const running = await page.evaluate(
          () =>
            document
              .getAnimations()
              .filter(
                (a) =>
                  a.playState === "running" &&
                  Number(a.effect?.getComputedTiming().activeDuration) > 150
              ).length
        )
        expect(running).toBe(0)
      }
    })
  }
}
