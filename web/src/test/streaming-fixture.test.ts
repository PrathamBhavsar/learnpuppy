import { describe, expect, it } from "vitest"

import {
  STREAMING_CHUNKS,
  STREAMING_TEXT,
} from "../../e2e/fixtures/streaming-message"

describe("streaming fixture", () => {
  it("has 12 chunks covering fence, math, table and mermaid", () => {
    expect(STREAMING_CHUNKS).toHaveLength(12)
    expect(STREAMING_TEXT).toContain("```rust")
    expect(STREAMING_TEXT).toContain("$$ x^2 + 1 $$")
    expect(STREAMING_TEXT).toContain("| --- | --- |")
    expect(STREAMING_TEXT).toContain("```mermaid")
  })
})
