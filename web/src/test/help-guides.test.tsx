import { screen, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it } from "vitest"

import { renderApp } from "@/test/render"

const cases: [string, string, string][] = [
  ["/", "Workspaces", "Workspace cards"],
  ["/trash", "Trash", "Restore"],
  ["/settings", "Settings", "Appearance"],
  ["/accounts", "Accounts", "Cooldown ring"],
  ["/w/ws_rustasync/settings", "Workspace settings", "Model overrides"],
]

describe("About this screen", () => {
  it.each(cases)("%s lists its parts", async (path, title, part) => {
    renderApp(path)
    await userEvent.click(await screen.findByTestId("screen-guide-open"))
    const guide = await screen.findByTestId("screen-guide")
    expect(
      within(guide).getByRole("heading", { name: title })
    ).toBeInTheDocument()
    expect(guide).toHaveTextContent(part)
  })

  it("the Help menu opens the app-frame guide", async () => {
    renderApp("/w/ws_rustasync/graph")
    await userEvent.click(await screen.findByTestId("help-menu"))
    await userEvent.click(await screen.findByTestId("help-guide"))
    expect(await screen.findByTestId("screen-guide")).toHaveTextContent(
      "Workspace switcher"
    )
  })
})
