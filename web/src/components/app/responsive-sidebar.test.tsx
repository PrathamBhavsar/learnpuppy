import { screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"

import { mockMatchMedia } from "@/test/media"
import { renderWithProviders } from "@/test/render"
import { useUi } from "@/stores/ui"

import { ResponsiveSidebar } from "./responsive-sidebar"

function Navigation({
  onToggle,
}: {
  collapsed: boolean
  onToggle: () => void
}) {
  return (
    <aside data-testid="sidebar">
      <a href="#lesson">Lesson</a>
      <button onClick={onToggle}>Close navigation</button>
    </aside>
  )
}

describe("responsive sidebar", () => {
  it("uses a modal drawer below 928px and restores trigger focus on Escape", async () => {
    mockMatchMedia({ "(max-width: 927px)": true })
    const user = userEvent.setup()
    renderWithProviders(
      <ResponsiveSidebar>
        <Navigation collapsed={false} onToggle={vi.fn()} />
      </ResponsiveSidebar>
    )
    expect(screen.queryByTestId("sidebar")).not.toBeInTheDocument()
    const trigger = screen.getByRole("button", { name: "Open sidebar" })
    await user.click(trigger)
    const drawer = screen.getByRole("dialog", { name: "Navigation" })
    for (let i = 0; i < 5; i++) await user.tab()
    expect(drawer.contains(document.activeElement)).toBe(true)
    await user.keyboard("{Escape}")
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument()
    )
    expect(trigger).toHaveFocus()
    await user.keyboard("{Control>}b{/Control}")
    expect(
      screen.getByRole("dialog", { name: "Navigation" })
    ).toBeInTheDocument()
    await user.click(screen.getByText("Close navigation"))
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument()
    )
  })

  it("keeps desktop navigation inline and binds Ctrl+B to its preference", async () => {
    useUi.setState({ sidebarCollapsed: false })
    const user = userEvent.setup()
    renderWithProviders(
      <ResponsiveSidebar>
        <Navigation collapsed={false} onToggle={vi.fn()} />
      </ResponsiveSidebar>
    )
    expect(screen.getByTestId("sidebar")).toBeInTheDocument()
    expect(
      screen.queryByRole("button", { name: "Open sidebar" })
    ).not.toBeInTheDocument()
    await user.keyboard("{Control>}b{/Control}")
    expect(useUi.getState().sidebarCollapsed).toBe(true)
  })
})
