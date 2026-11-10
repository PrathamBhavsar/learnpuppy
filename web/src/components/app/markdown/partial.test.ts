import { describe, expect, it } from "vitest"

import { completePartial } from "./partial"

const CASES: [string, string, string][] = [
  ["open fence", "```rust\nfn a()", "```rust\nfn a()\n```"],
  ["open fence with trailing newline", "```js\nx\n", "```js\nx\n```"],
  ["tilde fence", "~~~\nx", "~~~\nx\n~~~"],
  ["closed fence untouched", "```js\nx\n```\nafter", "```js\nx\n```\nafter"],
  [
    "inside fence untouched",
    "```md\n**a $$ `b [c](\n",
    "```md\n**a $$ `b [c](\n```",
  ],
  ["odd $$", "see $$ x^2 + y", "see $$ x^2 + y\n$$"],
  ["bare $$ opener dropped", "see $$", "see "],
  ["even $$ untouched", "$$ x $$ done", "$$ x $$ done"],
  ["single $", "so $x + 1", "so $x + 1$"],
  ["currency untouched", "costs $5", "costs $5"],
  ["bold", "a **bol", "a **bol**"],
  ["bare ** opener dropped", "a **", "a "],
  ["bold closed untouched", "a **b** c", "a **b** c"],
  ["backtick", "use `foo", "use `foo`"],
  ["bold inside code ignored", "run `a ** b` now", "run `a ** b` now"],
  ["dangling link", "read [the docs](https://ex", "read the docs"],
  ["full link untouched", "[d](https://e.com) x", "[d](https://e.com) x"],
  ["nested list bold", "- a\n  - **b", "- a\n  - **b**"],
  ["plain text untouched", "hello world", "hello world"],
]

describe("completePartial", () => {
  it.each(CASES)("%s", (_name, input, expected) => {
    expect(completePartial(input)).toBe(expected)
  })

  it("is a no-op on settled text", () => {
    const settled = "A **b** `c` $d$\n\n```x\ny\n```\n"
    expect(completePartial(settled)).toBe(settled)
  })
})
