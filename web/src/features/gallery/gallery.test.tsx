import { screen, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it } from "vitest"

import { renderApp, resetUi } from "@/test/render"

beforeEach(resetUi)

const SECTIONS = [
  "matrix",
  "buttons",
  "forms",
  "overlays",
  "data",
  "app",
  "states",
  "motion",
  "palette",
]

describe("gallery", () => {
  it("renders every section", async () => {
    renderApp("/gallery")
    await screen.findByTestId("gallery")
    for (const id of SECTIONS)
      expect(screen.getByTestId(`gallery-${id}`)).toBeInTheDocument()
  })

  it("shows the theme/direction matrix in all four combinations", async () => {
    renderApp("/gallery")
    for (const theme of ["dark", "light"]) {
      for (const dir of ["ltr", "rtl"]) {
        const quadrant = await screen.findByTestId(`matrix-${theme}-${dir}`)
        expect(quadrant).toHaveClass(theme)
        expect(quadrant).toHaveAttribute("dir", dir)
      }
    }
  })

  it("lists a swatch for every semantic token family", async () => {
    renderApp("/gallery")
    await screen.findByTestId("gallery-palette")
    const palette = within(screen.getByTestId("gallery-palette"))
    for (const token of [
      "success",
      "warning",
      "info",
      "mastery-0",
      "mastery-4",
      "source-claude",
      "source-git",
      "cluster-1",
      "cluster-8",
    ]) {
      expect(palette.getByTestId(`swatch-${token}`)).toBeInTheDocument()
    }
  })

  it("opens a dialog from the gallery", async () => {
    const user = userEvent.setup()
    renderApp("/gallery")
    await user.click(await screen.findByTestId("gallery-dialog-trigger"))
    expect(
      await screen.findByRole("dialog", { name: "Rename workspace" })
    ).toBeInTheDocument()
  })

  it("validates the form with zod and saves after a valid submit", async () => {
    const user = userEvent.setup()
    renderApp("/gallery")
    await screen.findByTestId("gallery-form")
    await user.type(screen.getByLabelText("Name"), "a")
    await user.click(screen.getByTestId("gallery-form-submit"))
    expect(
      await screen.findByText("Use at least 2 characters")
    ).toBeInTheDocument()
    expect(screen.getByLabelText("Name")).toHaveAttribute(
      "aria-invalid",
      "true"
    )

    await user.type(screen.getByLabelText("Name"), "bc")
    await user.click(screen.getByTestId("gallery-form-submit"))
    expect(await screen.findByTestId("gallery-form-saved")).toHaveTextContent(
      "Saved abc"
    )
    expect(screen.getByLabelText("Name")).toHaveValue("")
  })

  it("CountUp in the gallery advances when the button is pressed", async () => {
    const user = userEvent.setup()
    renderApp("/gallery")
    await screen.findByTestId("gallery-countup")
    await user.click(screen.getByTestId("gallery-countup-add"))
    await screen.findByText("165", undefined, { timeout: 2000 })
  })
})
