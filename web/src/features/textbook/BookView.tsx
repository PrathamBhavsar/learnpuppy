import { ArrowLeftIcon } from "lucide-react"
import { useState } from "react"

import { toApiError } from "@/api/errors"
import { ErrorState } from "@/components/app/error-state"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonRows } from "@/components/app/skeleton-block"
import { Button } from "@/components/ui/button"
import { toastError } from "@/lib/toast"

import { type Chapter, useAbort, useBook, useImport, useJobs } from "./api"
import { ChapterList } from "./ChapterList"
import { copy, errorText } from "./copy"
import { ImportProgress } from "./ImportProgress"
import { ReadingCheck } from "./ReadingCheck"

/** One book: import progress, the chapter list, and the reading check for the chapter being tested. */
export function BookView({
  ws,
  id,
  onBack,
}: {
  ws: string
  id: string
  onBack: () => void
}) {
  const book = useBook(ws, id)
  const importing = Boolean(book.data?.jobId)
  const jobs = useJobs(ws, importing)
  const run = useImport(ws, id)
  const abort = useAbort(ws)
  const [open, setOpen] = useState<number | null>(null)

  if (book.isError && !book.data)
    return (
      <ErrorState
        message={toApiError(book.error).message}
        requestId={toApiError(book.error).requestId}
        onRetry={() => book.refetch()}
      />
    )
  if (!book.data)
    return (
      <div data-testid="book-loading" aria-busy>
        <SkeletonRows rows={6} />
      </div>
    )

  const b = book.data
  const job = jobs.data?.find((j) => j.kind === "textbook" && j.bookId === id)
  const unfinished = b.chapters.filter(
    (c) => c.state === "failed" || c.state === "queued"
  )
  const checking: Chapter | undefined =
    open === null ? undefined : b.chapters.find((c) => c.index === open)

  const importChapters = (chapters?: number[]) =>
    run.mutate(chapters, {
      onError: (e) => toastError(e, "Could not start the import"),
    })

  return (
    <div data-testid="book-view" className="flex flex-col gap-4">
      <Button
        variant="ghost"
        size="sm"
        className="self-start"
        data-testid="book-back"
        onClick={onBack}
      >
        <ArrowLeftIcon /> All textbooks
      </Button>
      <PageHeader
        title={b.title}
        description={`${b.totalPages} pages, ${b.chapters.length} chapters`}
      />
      {importing ? (
        <ImportProgress
          job={job}
          cancelling={abort.isPending}
          onCancel={() => job && abort.mutate(job.id)}
        />
      ) : null}
      {!importing && unfinished.length > 0 ? (
        <div
          role="alert"
          data-testid="book-partial"
          className="flex items-center justify-between gap-3 border border-destructive/40 p-3 text-[13px]"
        >
          <span>
            {unfinished.length === 1
              ? "1 chapter is not imported."
              : `${unfinished.length} chapters are not imported.`}{" "}
            {unfinished[0]?.error ? errorText(unfinished[0].error) : null}
          </span>
          <Button
            size="sm"
            data-testid="book-import-rest"
            disabled={run.isPending}
            onClick={() => importChapters()}
          >
            Import the rest
          </Button>
        </div>
      ) : null}
      {checking ? (
        <ReadingCheck
          key={checking.index}
          ws={ws}
          book={id}
          chapter={checking}
          onClose={() => setOpen(null)}
        />
      ) : b.chapters.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">{copy.noChapters}</p>
      ) : (
        <ChapterList
          chapters={b.chapters}
          busy={run.isPending}
          onCheck={(c) => setOpen(c.index)}
          onRetry={(c) => importChapters([c.index])}
        />
      )}
    </div>
  )
}
