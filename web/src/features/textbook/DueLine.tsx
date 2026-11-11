import { BookOpenIcon } from "lucide-react"

import { useBooksDue } from "./api"

/** One line for the digest: how many chapters are due for a reading check. Hidden when none are. */
export function TextbookDueLine({
  ws,
  onOpen,
}: {
  ws: string
  onOpen: () => void
}) {
  const due = useBooksDue(ws)
  if (due === 0) return null
  return (
    <p
      data-testid="digest-chapters-due"
      className="flex items-center gap-2 text-[13px]"
    >
      <BookOpenIcon className="size-4 text-muted-foreground" aria-hidden />
      <span>
        {due === 1 ? "1 chapter is due" : `${due} chapters are due`} for a
        reading check.
      </span>
      <button
        type="button"
        className="underline underline-offset-2"
        onClick={onOpen}
      >
        Open textbooks
      </button>
    </p>
  )
}
