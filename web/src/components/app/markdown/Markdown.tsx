import { type MarkdownViewProps, MarkdownView } from "./MarkdownView"

/** Markdown without math: the common case, and the cheap chunk. */
export default function Markdown(props: MarkdownViewProps) {
  return <MarkdownView {...props} />
}
