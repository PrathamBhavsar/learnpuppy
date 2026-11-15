import { BookOpenIcon } from "lucide-react"

import { toApiError } from "@/api/errors"
import { EmptyState } from "@/components/app/empty-state"
import { ErrorState } from "@/components/app/error-state"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonRows } from "@/components/app/skeleton-block"
import { Badge } from "@/components/ui/badge"
import { toastError, toastSuccess } from "@/lib/toast"

import { useBooks, useUpload } from "./api"
import { BookView } from "./BookView"
import { copy } from "./copy"
import { UploadCard } from "./UploadCard"

const STATE_LABEL = {
  importing: "Importing",
  ready: "Ready",
  partial: "Partly imported",
  failed: "Failed",
} as const

type Props = {
  ws: string
  /** The open book, kept in the URL. */
  book: string | undefined
  /** Opened from the palette: put focus on the upload button. */
  focusUpload?: boolean
  onOpen: (book: string | undefined) => void
}

/** Textbook list with upload; opening a book shows its chapters. */
export function TextbookScreen({ ws, book, focusUpload, onOpen }: Props) {
  const books = useBooks(ws)
  const upload = useUpload(ws)

  if (book)
    return (
      <div data-testid="screen-textbook" className="mx-auto max-w-2xl p-6">
        <BookView ws={ws} id={book} onBack={() => onOpen(undefined)} />
      </div>
    )

  const send = (file: File) =>
    upload.mutate(file, {
      onSuccess: (u) => {
        if (u.existing) toastSuccess(copy.existing)
        onOpen(u.bookId)
      },
      onError: (e) => toastError(e, "Could not upload the PDF"),
    })

  return (
    <div
      data-testid="screen-textbook"
      aria-busy={books.isPending}
      className="mx-auto flex max-w-2xl flex-col gap-6 p-6"
    >
      <PageHeader title={copy.title} description={copy.description} />
      <UploadCard
        pending={upload.isPending}
        focus={focusUpload}
        onFile={send}
      />
      {books.isError && !books.data ? (
        <ErrorState
          message={toApiError(books.error).message}
          requestId={toApiError(books.error).requestId}
          onRetry={() => books.refetch()}
        />
      ) : books.isPending ? (
        <div data-testid="textbook-loading">
          <SkeletonRows rows={3} />
        </div>
      ) : books.data.length === 0 ? (
        <EmptyState
          icon={BookOpenIcon}
          title={copy.emptyTitle}
          description={copy.emptyBody}
          focusAction={false}
        />
      ) : (
        <ul data-testid="textbook-list" className="flex flex-col gap-1">
          {books.data.map((b) => (
            <li key={b.id}>
              <button
                type="button"
                data-testid={`textbook-${b.id}`}
                onClick={() => onOpen(b.id)}
                className="flex w-full items-center justify-between gap-3 border px-3 py-2 text-start text-[13px] hover:bg-muted/50"
              >
                <span className="flex min-w-0 flex-col">
                  <span className="truncate font-medium">{b.title}</span>
                  <span className="text-[12px] text-muted-foreground">
                    {b.ready} of {b.chapters} chapters ready, {b.totalPages}{" "}
                    pages
                  </span>
                </span>
                <Badge variant="outline">{STATE_LABEL[b.state]}</Badge>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
