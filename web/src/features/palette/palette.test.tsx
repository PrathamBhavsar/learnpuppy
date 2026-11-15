import { act, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it } from "vitest"

import { usePalette } from "@/stores/palette"
import { state } from "@/test/msw/db"
import { useUi } from "@/stores/ui"
import { renderApp, resetUi } from "@/test/render"

const RUST = "ws_rustasync"

beforeEach(resetUi)

async function openShell(path = `/w/${RUST}/graph`) {
  const view = renderApp(path)
  await screen.findByRole("button", { name: /Workspace: Rust async/ })
  return view
}

const pressModK = () =>
  act(async () => {
    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "k", metaKey: true, ctrlKey: true })
    )
  })

describe("command palette", () => {
  it("opens with the hotkey, lists the groups from the spec, and closes on Escape", async () => {
    const user = userEvent.setup()
    await openShell()
    await pressModK()
    const input = await screen.findByTestId("palette-input")
    expect(input).toHaveFocus()
    for (const group of ["navigate", "workspaces", "actions", "appearance"]) {
      expect(
        await screen.findByTestId(`palette-group-${group}`)
      ).toBeInTheDocument()
    }
    await user.keyboard("{Escape}")
    await waitFor(() =>
      expect(screen.queryByTestId("palette-input")).not.toBeInTheDocument()
    )
  })

  it("filters as you type and runs the highlighted command with Enter", async () => {
    const user = userEvent.setup()
    const { router } = await openShell()
    await pressModK()
    await user.type(await screen.findByTestId("palette-input"), "go to rev")
    expect(
      screen.queryByTestId("palette-cmd-go-ingest")
    ).not.toBeInTheDocument()
    await user.keyboard("{Enter}")
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(`/w/${RUST}/review`)
    )
    expect(usePalette.getState().open).toBe(false)
    expect(usePalette.getState().query).toBe("")
  })

  it("moves with the arrow keys", async () => {
    const user = userEvent.setup()
    useUi.setState({ legacyClassroom: true })
    const { router } = await openShell()
    await pressModK()
    await screen.findByTestId("palette-input")
    await user.keyboard("{ArrowDown}{ArrowDown}{Enter}")
    await waitFor(() => expect(usePalette.getState().open).toBe(false))
    // The first two navigate commands are Graph and Learn; the third is Review.
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(`/w/${RUST}/review`)
    )
  })

  it("hides Go to Learn unless the legacy classroom is on", async () => {
    await openShell()
    await pressModK()
    await screen.findByTestId("palette-input")
    expect(screen.getByTestId("palette-cmd-go-graph")).toBeInTheDocument()
    expect(screen.queryByTestId("palette-cmd-go-learn")).not.toBeInTheDocument()
  })

  it("remembers recents and shows them first on the next open", async () => {
    const user = userEvent.setup()
    await openShell()
    await pressModK()
    await user.click(await screen.findByTestId("palette-cmd-go-digest"))
    await pressModK()
    const recent = await screen.findByTestId("palette-group-recent")
    expect(
      within(recent).getByTestId("palette-cmd-go-digest")
    ).toBeInTheDocument()
    expect(
      JSON.parse(localStorage.getItem("lp-palette-recents") ?? "[]")
    ).toEqual(["go-digest"])
  })

  it("searches nodes live and jumps to the graph with the node selected", async () => {
    const user = userEvent.setup()
    const { router } = await openShell("/w/ws_rustasync/digest")
    const node = state.db.graphs.get(RUST)?.snapshot.nodes[0]
    if (!node) throw new Error("fixture graph is empty")
    await pressModK()
    await user.type(
      await screen.findByTestId("palette-input"),
      node.text.slice(0, 18)
    )
    const hit = await screen.findByTestId(`palette-node-${node.id}`)
    await user.click(hit)
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(`/w/${RUST}/graph`)
    )
    expect(router.state.location.searchStr).toContain(node.id)
  })

  it("Tab widens node search to every workspace", async () => {
    const user = userEvent.setup()
    await openShell()
    await pressModK()
    await screen.findByTestId("palette-input")
    expect(screen.getByTestId("palette-scope")).toHaveTextContent(
      "this workspace"
    )
    await user.keyboard("{Tab}")
    expect(screen.getByTestId("palette-scope")).toHaveTextContent(
      "all workspaces"
    )
    await user.keyboard("{Tab}")
    expect(screen.getByTestId("palette-scope")).toHaveTextContent(
      "this workspace"
    )
  })

  it("offers a switch command per workspace and keeps the current section", async () => {
    const user = userEvent.setup()
    const { router } = await openShell(`/w/${RUST}/review`)
    await pressModK()
    await user.type(
      await screen.findByTestId("palette-input"),
      "switch to distributed"
    )
    await user.keyboard("{Enter}")
    await waitFor(() =>
      expect(router.state.location.pathname).toBe("/w/ws_distsys/review")
    )
  })

  it("lists recent chats of the workspace", async () => {
    await openShell()
    await pressModK()
    expect(await screen.findByTestId("palette-group-chats")).toBeInTheDocument()
  })
})

describe("shortcuts sheet and global chords", () => {
  it("? opens a sheet generated from the registry", async () => {
    const user = userEvent.setup()
    await openShell()
    await user.keyboard("?")
    const sheet = await screen.findByTestId("shortcuts-sheet")
    expect(within(sheet).getByTestId("shortcut-palette")).toBeInTheDocument()
    expect(within(sheet).getByTestId("shortcut-go-review")).toHaveTextContent(
      "Go to Review"
    )
    expect(within(sheet).getByTestId("shortcut-sidebar")).toBeInTheDocument()
    expect(within(sheet).getByTestId("shortcut-workspace-1")).toHaveTextContent(
      "Switch to"
    )
    await user.keyboard("{Escape}")
    await waitFor(() =>
      expect(screen.queryByTestId("shortcuts-sheet")).not.toBeInTheDocument()
    )
  })

  it("g then r goes to Review; mod+2 switches workspace", async () => {
    const user = userEvent.setup()
    const { router } = await openShell()
    await user.keyboard("gr")
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(`/w/${RUST}/review`)
    )
    await act(async () => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", { key: "2", metaKey: true, ctrlKey: true })
      )
    })
    await waitFor(() =>
      expect(router.state.location.pathname).toMatch(/^\/w\/ws_[a-z]+\/review$/)
    )
    expect(router.state.location.pathname).not.toContain(RUST)
  })
})
