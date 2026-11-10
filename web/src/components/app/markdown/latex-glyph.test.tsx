import { render, screen, waitFor } from "@testing-library/react"
import { describe, expect, it } from "vitest"

import { LatexGlyph } from "./latex-glyph"

describe("LatexGlyph", () => {
  it("exposes the plain sentence and renders KaTeX markup hidden from AT", async () => {
    const { container } = render(
      <LatexGlyph tex="\implies" label="implies" fallback="⟹" />
    )
    expect(screen.getByRole("img", { name: "implies" })).toBeTruthy()
    await waitFor(() => expect(container.querySelector(".katex")).toBeTruthy())
    expect(
      container.querySelector(".katex")?.closest("[aria-hidden]")
    ).toBeTruthy()
  })

  it("shows the fallback glyph first and caches the result", async () => {
    const first = render(<LatexGlyph tex="\land" label="and" fallback="∧" />)
    await waitFor(() =>
      expect(first.container.querySelector(".katex")).toBeTruthy()
    )
    first.unmount()
    const again = render(<LatexGlyph tex="\land" label="and" fallback="∧" />)
    expect(again.container.querySelector(".katex")).toBeTruthy()
  })
})
