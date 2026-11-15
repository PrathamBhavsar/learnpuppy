import { render, screen, waitFor } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import { MarkdownView } from "./MarkdownView"
import { sanitizeForTest } from "./mermaid-render"

const render_ = vi.fn()
vi.mock("mermaid", () => ({
  default: { initialize: vi.fn(), render: (...a: unknown[]) => render_(...a) },
}))

describe("MermaidBlock", () => {
  it("renders a valid diagram as SVG with an accessible label", async () => {
    render_.mockResolvedValue({ svg: '<svg data-x="ok"><g></g></svg>' })
    render(<MarkdownView>{"```mermaid\ngraph TD; A-->B\n```"}</MarkdownView>)
    const block = await screen.findByTestId("mermaid-block")
    await waitFor(() => expect(block.querySelector("svg")).not.toBeNull())
    expect(
      block.querySelector("[aria-label]")?.getAttribute("aria-label")
    ).toContain("A-->B")
    expect(block.textContent).toContain("A-->B")
  })

  it("shows an error card with the source and a copy button on a parse error", async () => {
    render_.mockRejectedValue(new Error("Parse error"))
    render(<MarkdownView>{"```mermaid\nnot a diagram\n```"}</MarkdownView>)
    const card = await screen.findByTestId("mermaid-error")
    expect(card.textContent).toContain("not a diagram")
    expect(screen.getByTestId("mermaid-copy")).toBeInTheDocument()
  })
})

describe("sanitizeSvg", () => {
  const cases: Record<string, [string, RegExp]> = {
    "click directive": [
      '<svg><a onclick="alert(1)" href="javascript:alert(1)"><text>x</text></a></svg>',
      /onclick|javascript:/i,
    ],
    "foreignObject script": [
      "<svg><foreignObject><script>alert(1)</script><div>hi</div></foreignObject></svg>",
      /<script|foreignObject/i,
    ],
    "javascript link": [
      '<svg><a xlink:href="javascript:alert(1)"><text>x</text></a></svg>',
      /javascript:/i,
    ],
    "img onerror label": [
      '<svg><foreignObject><img src=x onerror="alert(1)"></foreignObject></svg>',
      /onerror|<img/i,
    ],
  }
  for (const [name, [input, bad]] of Object.entries(cases)) {
    it(`neutralises ${name}`, async () => {
      expect(await sanitizeForTest(input)).not.toMatch(bad)
    })
  }
})
