import { screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it } from "vitest"

import { mockMatchMedia } from "@/test/media"
import { renderWithProviders, resetUi } from "@/test/render"
import { useUi } from "@/stores/ui"

import { DirectionToggle } from "./direction-toggle"
import { ThemeToggle } from "./theme-toggle"

beforeEach(resetUi)

describe("ThemeToggle", () => {
  it("switches to light and dark, persists, and updates <html>", async () => {
    const user = userEvent.setup()
    renderWithProviders(<ThemeToggle />)

    await user.click(screen.getByTestId("theme-toggle"))
    await user.click(await screen.findByTestId("theme-option-light"))
    expect(document.documentElement).toHaveClass("light")
    expect(document.documentElement).not.toHaveClass("dark")
    expect(localStorage.getItem("lp-theme")).toBe("light")

    await user.click(screen.getByTestId("theme-toggle"))
    await user.click(await screen.findByTestId("theme-option-dark"))
    expect(document.documentElement).toHaveClass("dark")
    expect(localStorage.getItem("lp-theme")).toBe("dark")
    expect(screen.getByTestId("theme-toggle")).toHaveAccessibleName(
      "Theme: Dark"
    )
  })

  it("tracks the OS color scheme while set to system", async () => {
    const setPrefers = mockMatchMedia({ "(prefers-color-scheme: dark)": false })
    const user = userEvent.setup()
    renderWithProviders(<ThemeToggle />)
    await user.click(screen.getByTestId("theme-toggle"))
    await user.click(await screen.findByTestId("theme-option-system"))
    expect(document.documentElement).toHaveClass("light")

    setPrefers("(prefers-color-scheme: dark)", true)
    await waitFor(() => expect(document.documentElement).toHaveClass("dark"))
    expect(useUi.getState().resolvedTheme).toBe("dark")
  })
})

describe("DirectionToggle", () => {
  it("flips <html dir> and remembers the choice", async () => {
    const user = userEvent.setup()
    renderWithProviders(<DirectionToggle />)
    const button = screen.getByTestId("direction-toggle")
    expect(button).toHaveTextContent("LTR")

    await user.click(button)
    expect(document.documentElement.dir).toBe("rtl")
    expect(localStorage.getItem("lp-dir")).toBe("rtl")
    expect(button).toHaveTextContent("RTL")

    await user.click(button)
    expect(document.documentElement.dir).toBe("ltr")
  })
})
