import { render, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it } from "vitest"

import { resolveLanguage } from "./markdown/highlight"
import { LazyMarkdown } from "./markdown/LazyMarkdown"
import Markdown from "./markdown/Markdown"
import MarkdownMath from "./markdown/MarkdownMath"

describe("Markdown", () => {
  it("opens links externally with noopener", () => {
    const { container } = render(
      <Markdown>{"[docs](https://example.com)"}</Markdown>
    )
    const link = container.querySelector("a")
    expect(link).toHaveAttribute("href", "https://example.com")
    expect(link).toHaveAttribute("target", "_blank")
    expect(link?.getAttribute("rel")).toContain("noopener")
  })

  it("renders GFM tables and math", () => {
    const { container } = render(
      <MarkdownMath>
        {"| a | b |\n|---|---|\n| 1 | 2 |\n\nInline $x^2$ and\n\n$$\\sum_i i$$"}
      </MarkdownMath>
    )
    expect(container.querySelector("table")).not.toBeNull()
    expect(container.querySelectorAll(".katex").length).toBeGreaterThanOrEqual(
      2
    )
  })

  it("keeps inline code inline and fences as code blocks", () => {
    const { container } = render(
      <Markdown>{"Use `poll` here.\n\n```rust\nlet x = 1;\n```"}</Markdown>
    )
    expect(container.querySelectorAll("[data-testid=code-block]")).toHaveLength(
      1
    )
    expect(container.querySelector("p code")?.textContent).toBe("poll")
  })

  it("highlights known languages once shiki has loaded them", async () => {
    const { container } = render(
      <Markdown>{"```rust\nlet x = 1;\n```"}</Markdown>
    )
    expect(container.querySelector("pre")).not.toBeNull()
    await waitFor(
      () => expect(container.querySelector(".lp-shiki .shiki")).not.toBeNull(),
      {
        timeout: 8000,
      }
    )
    expect(
      container.querySelector(".shiki span[style]")?.getAttribute("style")
    ).toContain("--shiki-light")
  })

  it("leaves unknown languages as plain text", async () => {
    const { container } = render(
      <Markdown>{"```brainfuck\n+++\n```"}</Markdown>
    )
    await new Promise((r) => setTimeout(r, 50))
    expect(container.querySelector(".shiki")).toBeNull()
    expect(container.textContent).toContain("+++")
  })

  it("does not execute raw HTML", () => {
    const { container } = render(
      <Markdown>{'<img src=x onerror="alert(1)">'}</Markdown>
    )
    expect(container.querySelector("img")).toBeNull()
  })

  it("copies code", async () => {
    const user = userEvent.setup()
    render(<Markdown>{"```\nhello\n```"}</Markdown>)
    await user.click(screen.getByTestId("code-copy"))
    expect(await navigator.clipboard.readText()).toBe("hello")
  })
})

describe("LazyMarkdown", () => {
  it("only loads math rendering for messages that contain math", async () => {
    const { container, rerender } = render(
      <LazyMarkdown>{"cost is plain"}</LazyMarkdown>
    )
    await waitFor(() =>
      expect(container.querySelector(".lp-prose")).not.toBeNull()
    )
    expect(container.querySelector(".katex")).toBeNull()
    rerender(<LazyMarkdown>{"energy $E=mc^2$"}</LazyMarkdown>)
    await waitFor(() =>
      expect(container.querySelector(".katex")).not.toBeNull()
    )
  })

  it("shows the raw text while the renderer loads, then the rendered Markdown", async () => {
    const { container } = render(<LazyMarkdown>{"**bold**"}</LazyMarkdown>)
    expect(container.textContent).toContain("bold")
    await waitFor(() =>
      expect(container.querySelector("strong")).not.toBeNull()
    )
  })
})

describe("resolveLanguage", () => {
  it("maps aliases and rejects what is not bundled", () => {
    expect(resolveLanguage("rs")).toBe("rust")
    expect(resolveLanguage("SH")).toBe("bash")
    expect(resolveLanguage("c++")).toBe("cpp")
    expect(resolveLanguage("cobol")).toBeNull()
    expect(resolveLanguage(undefined)).toBeNull()
  })
})
