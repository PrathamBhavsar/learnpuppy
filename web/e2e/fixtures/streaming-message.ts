// One assistant message split into 12 chunks. Joined, it holds a fenced code block,
// display math, a table and a Mermaid block. U1 and U2 stream it to prove no raw syntax flashes.
export const STREAMING_CHUNKS: readonly string[] = [
  "Here is the idea.\n\n```rust\nfn ",
  'main() {\n    println!("hi");\n}\n```\n\n',
  "Energy grows as $$ x^2 ",
  "+ 1 $$ for large x.\n\n",
  "| name | cost |\n| --- | --- |\n",
  "| a | **1** |\n| b | 2 |\n\n",
  "Flow:\n\n```mermaid\n",
  "graph TD\n  A[Ask] --> B[Think]\n",
  "  B --> C[Answer]\n```\n\n",
  "Use `cargo test` ",
  "and read [the docs](https://doc.rust-lang.org) ",
  "when done.",
]

export const STREAMING_TEXT = STREAMING_CHUNKS.join("")
