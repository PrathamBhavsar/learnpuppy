import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { useHotkey, useHotkeys } from "@/hooks/use-hotkey"

import { CHORD_WINDOW_MS, getShortcuts, resetChord, splitBinding } from "./keys"

function Chords({
  go,
  enabled = true,
}: {
  go: (to: string) => void
  enabled?: boolean
}) {
  useHotkey("g l", () => go("learn"), {
    id: "t-learn",
    label: "Go to Learn",
    group: "Go to",
    enabled,
  })
  useHotkey("g r", () => go("review"), {
    id: "t-review",
    label: "Go to Review",
    group: "Go to",
  })
  useHotkey("mod+k", () => go("palette"), {
    id: "t-palette",
    label: "Palette",
    group: "General",
  })
  useHotkey("?", () => go("help"), {
    id: "t-help",
    label: "Help",
    group: "General",
  })
  return <input aria-label="Field" />
}

beforeEach(() => resetChord())
afterEach(() => vi.useRealTimers())

describe("hotkey chords", () => {
  it("fires on the second key of a vim-style chord", async () => {
    const go = vi.fn()
    const user = userEvent.setup()
    render(<Chords go={go} />)
    await user.keyboard("gl")
    expect(go).toHaveBeenCalledExactlyOnceWith("learn")
    await user.keyboard("gr")
    expect(go).toHaveBeenLastCalledWith("review")
  })

  it("does nothing for an unknown second key, and the prefix does not stay armed", async () => {
    const go = vi.fn()
    const user = userEvent.setup()
    render(<Chords go={go} />)
    await user.keyboard("gx")
    await user.keyboard("l")
    expect(go).not.toHaveBeenCalled()
  })

  it("expires the prefix after 800 ms", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const go = vi.fn()
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    render(<Chords go={go} />)
    await user.keyboard("g")
    vi.advanceTimersByTime(CHORD_WINDOW_MS + 50)
    await user.keyboard("l")
    expect(go).not.toHaveBeenCalled()
  })

  it("ignores plain chords while typing but still honours mod combos", async () => {
    const go = vi.fn()
    const user = userEvent.setup()
    render(<Chords go={go} />)
    await user.click(screen.getByLabelText("Field"))
    await user.keyboard("gl")
    await user.keyboard("?")
    expect(go).not.toHaveBeenCalled()
    await user.keyboard("{Control>}k{/Control}")
    await user.keyboard("{Meta>}k{/Meta}")
    expect(go).toHaveBeenCalledWith("palette")
  })

  it("does not fire disabled bindings", async () => {
    const go = vi.fn()
    const user = userEvent.setup()
    render(<Chords go={go} enabled={false} />)
    await user.keyboard("gl")
    expect(go).not.toHaveBeenCalled()
  })

  it("treats a lone key as a normal binding when it is not a chord prefix", async () => {
    const go = vi.fn()
    const user = userEvent.setup()
    render(<Chords go={go} />)
    await user.keyboard("?")
    expect(go).toHaveBeenCalledWith("help")
  })
})

describe("shortcut registry", () => {
  it("lists labelled bindings and drops them on unmount", () => {
    const view = render(<Chords go={() => undefined} />)
    const ids = getShortcuts().map((s) => s.id)
    expect(ids).toEqual(
      expect.arrayContaining(["t-learn", "t-review", "t-palette", "t-help"])
    )
    view.unmount()
    expect(getShortcuts().map((s) => s.id)).not.toContain("t-learn")
  })

  it("hides unlabeled bindings from the sheet", () => {
    function Silent() {
      useHotkey("x", () => undefined, { id: "t-silent" })
      return null
    }
    render(<Silent />)
    expect(getShortcuts().find((s) => s.id === "t-silent")?.hidden).toBe(true)
  })

  it("registers dynamic lists", () => {
    function Many({ names }: { names: string[] }) {
      useHotkeys(
        names.map((name, i) => ({
          id: `t-ws-${i}`,
          keys: `mod+${i + 1}`,
          label: `Switch to ${name}`,
          handler: () => undefined,
        }))
      )
      return null
    }
    const view = render(<Many names={["A", "B"]} />)
    expect(
      getShortcuts()
        .filter((s) => s.id.startsWith("t-ws-"))
        .map((s) => s.label)
    ).toEqual(["Switch to A", "Switch to B"])
    view.rerender(<Many names={["A"]} />)
    expect(getShortcuts().filter((s) => s.id.startsWith("t-ws-"))).toHaveLength(
      1
    )
  })

  it("splits bindings into display steps", () => {
    expect(splitBinding("g l")).toEqual([["g"], ["l"]])
    expect(splitBinding("mod+shift+k")).toEqual([["mod", "shift", "k"]])
  })
})
