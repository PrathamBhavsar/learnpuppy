import { render } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"

import { matchesCombo, useHotkey } from "./use-hotkey"

function Probe({
  onKey,
  enabled = true,
}: {
  onKey: () => void
  enabled?: boolean
}) {
  useHotkey("[", onKey, { enabled })
  return <input aria-label="Field" />
}

describe("matchesCombo", () => {
  it("matches plain keys and rejects stray modifiers", () => {
    expect(matchesCombo(new KeyboardEvent("keydown", { key: "[" }), "[")).toBe(
      true
    )
    expect(
      matchesCombo(
        new KeyboardEvent("keydown", { key: "[", ctrlKey: true }),
        "["
      )
    ).toBe(false)
    expect(
      matchesCombo(
        new KeyboardEvent("keydown", { key: "[", altKey: true }),
        "["
      )
    ).toBe(false)
    expect(matchesCombo(new KeyboardEvent("keydown", { key: "]" }), "[")).toBe(
      false
    )
  })

  it("treats mod as Ctrl or Cmd depending on the platform, and shift as required", () => {
    const ctrlOrMeta = (key: string, shift = false) =>
      [true, false].map((meta) =>
        matchesCombo(
          new KeyboardEvent("keydown", {
            key,
            metaKey: meta,
            ctrlKey: !meta,
            shiftKey: shift,
          }),
          "mod+shift+k"
        )
      )
    expect(ctrlOrMeta("k", true).filter(Boolean)).toHaveLength(1)
    expect(ctrlOrMeta("k", false).filter(Boolean)).toHaveLength(0)
  })
})

describe("useHotkey", () => {
  it("fires on the key, but not while typing in a field", async () => {
    const user = userEvent.setup()
    const onKey = vi.fn()
    const { getByLabelText } = render(<Probe onKey={onKey} />)

    await user.keyboard("[[")
    expect(onKey).toHaveBeenCalledTimes(1)

    await user.click(getByLabelText("Field"))
    await user.keyboard("[[")
    expect(onKey).toHaveBeenCalledTimes(1)
  })

  it("does nothing when disabled and cleans up on unmount", async () => {
    const user = userEvent.setup()
    const onKey = vi.fn()
    const { rerender, unmount } = render(
      <Probe onKey={onKey} enabled={false} />
    )
    await user.keyboard("[[")
    expect(onKey).not.toHaveBeenCalled()

    rerender(<Probe onKey={onKey} />)
    unmount()
    await user.keyboard("[[")
    expect(onKey).not.toHaveBeenCalled()
  })
})
