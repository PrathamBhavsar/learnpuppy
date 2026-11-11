import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"

import type { Chapter } from "./api"
import { chapterLabel, errorText, pageRange } from "./copy"

/** Chapter rows. Titles and error codes come from a PDF, so they render as plain text nodes only. */
export function ChapterList({
  chapters,
  busy,
  onCheck,
  onRetry,
}: {
  chapters: Chapter[]
  busy: boolean
  onCheck: (c: Chapter) => void
  onRetry: (c: Chapter) => void
}) {
  return (
    <ol data-testid="chapter-list" className="flex flex-col divide-y border">
      {chapters.map((c) => (
        <li
          key={c.index}
          data-testid={`chapter-${c.index}`}
          data-state={c.state}
          data-status={c.status}
          className="flex items-center justify-between gap-3 px-3 py-2"
        >
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="truncate text-[13px] font-medium">
              {c.index + 1}. {c.title}
            </span>
            <span className="text-[12px] text-muted-foreground">
              {pageRange(c)}
              {c.state === "ready" ? ` · ${c.objectives} objectives` : ""}
              {c.truncated ? " · long chapter, the end was left out" : ""}
            </span>
            {c.error ? (
              <span
                data-testid={`chapter-${c.index}-error`}
                className="text-[12px] text-destructive"
              >
                {errorText(c.error)}
              </span>
            ) : null}
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Badge
              variant={
                c.status === "due" && c.state === "ready"
                  ? "default"
                  : "outline"
              }
              data-testid={`chapter-${c.index}-badge`}
            >
              {chapterLabel(c)}
            </Badge>
            {c.state === "ready" ? (
              <Button
                size="sm"
                variant="outline"
                data-testid={`chapter-${c.index}-check`}
                aria-label={`Check I read chapter ${c.index + 1}`}
                disabled={busy}
                onClick={() => onCheck(c)}
              >
                Check I read
              </Button>
            ) : null}
            {c.state === "failed" || c.state === "queued" ? (
              <Button
                size="sm"
                variant="outline"
                data-testid={`chapter-${c.index}-retry`}
                aria-label={`Import chapter ${c.index + 1} again`}
                disabled={busy}
                onClick={() => onRetry(c)}
              >
                Import
              </Button>
            ) : null}
          </div>
        </li>
      ))}
    </ol>
  )
}
