import { render, waitFor } from "@testing-library/react"
import { expect, it } from "vitest"

import { PromptMarkdown } from "./prompt-markdown"

it("renders math and fenced code through the shared renderer", async () => {
  const { container } = render(
    <PromptMarkdown
      source={String.raw`Find \(x^2\)` + "\n\n```python\n    return 1\n```"}
    />
  )
  await waitFor(() => expect(container.querySelector(".katex")).not.toBeNull())
  expect(container.querySelector("pre code")?.textContent).toBe("    return 1")
})
