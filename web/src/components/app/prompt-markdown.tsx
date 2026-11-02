import { LazyMarkdown } from "@/components/app/markdown/LazyMarkdown"

/** Cards share the classroom's math, GFM, and code rendering. */
export function PromptMarkdown({
  source,
  className,
}: {
  source: string
  className?: string
}) {
  return <LazyMarkdown className={className}>{source}</LazyMarkdown>
}
