import { AnimatePresence, motion } from "motion/react"
import { useIsMutating } from "@tanstack/react-query"
import { type ReactNode, useCallback, useRef, useState } from "react"

import type { Rating } from "@/api/pending-types"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { useDirection } from "@/components/ui/direction"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { cn } from "@/lib/utils"

import { REVIEW_COPY } from "./copy"
import { ReviewCardView } from "./review-card"
import { type Session, gradedCount } from "./use-session"

const TINT: Record<Rating, string> = {
  1: "bg-destructive/15",
  2: "bg-warning/15",
  3: "bg-info/15",
  4: "bg-success/15",
}

type Props = {
  ws: string
  session: Session
  /** Shown above the progress bar (workspace name in /review-all). */
  heading?: ReactNode
  /** Rendered instead of the card once the queue drains. */
  summary: ReactNode
  /** Called after the user confirms leaving mid-session. */
  onExit: () => void
  /** Full-bleed focus mode (default). /review-all embeds the session instead. */
  focus?: boolean
}

/** Focus-mode session: progress bar, animated card, Esc-to-exit confirmation. Shared by Review and Review all. */
export function ReviewSession({
  ws,
  session,
  heading,
  summary,
  onExit,
  focus = true,
}: Props) {
  const reduced = useReducedMotion()
  const dir = useDirection()
  const [confirmExit, setConfirmExit] = useState(false)
  const focusRef = useRef<HTMLDivElement>(null)
  const { state } = session
  const requestExit = useCallback(() => setConfirmExit(true), [])
  const done = gradedCount(state)
  const pct = state.totalCards ? done / state.totalCards : 0
  const number = Math.min(done + 1, state.totalCards)
  const side = dir === "rtl" ? -1 : 1
  const saving = useIsMutating({ mutationKey: ["ws", ws, "review-grade"] }) > 0

  const content = (
    <div
      ref={focusRef}
      tabIndex={-1}
      data-testid="review-session"
      role="region"
      aria-label="Review session"
      className={cn(
        "flex flex-col outline-none",
        focus
          ? "fixed inset-0 z-40 overflow-y-auto bg-background p-6"
          : "flex-1"
      )}
    >
      {state.phase === "complete" ? (
        <fieldset disabled={saving} aria-busy={saving} className="contents">
          {saving ? (
            <p
              role="status"
              className="text-center text-[13px] text-muted-foreground"
            >
              Saving ratings…
            </p>
          ) : null}
          {summary}
        </fieldset>
      ) : (
        <div className="mx-auto flex w-full max-w-2xl flex-1 flex-col">
          {heading}
          <Button
            variant="ghost"
            size="sm"
            className="self-end"
            data-testid="review-exit"
            onClick={requestExit}
          >
            Exit review
          </Button>
          <div
            data-testid="review-progress"
            className="flex items-center gap-3 pb-4"
          >
            <span
              className="text-[12px] text-muted-foreground tabular-nums"
              data-testid="review-progress-label"
            >
              {REVIEW_COPY.questionOf(number, state.totalCards)}
            </span>
            <div
              role="progressbar"
              aria-label="Cards reviewed"
              aria-valuemin={0}
              aria-valuemax={state.totalCards}
              aria-valuenow={done}
              className="h-1.5 flex-1 overflow-hidden bg-muted"
            >
              <motion.div
                data-testid="review-progress-bar"
                className="h-full origin-left bg-primary rtl:origin-right"
                initial={false}
                animate={{ scaleX: pct }}
                transition={reduced ? { duration: 0 } : { duration: 0.22 }}
              />
            </div>
            <span className="text-[12px] text-muted-foreground tabular-nums">
              {Math.round(pct * 100)}%
            </span>
          </div>
          <div className="relative flex flex-1 items-start justify-center py-6">
            <AnimatePresence mode="popLayout" custom={state.lastRating}>
              {state.current ? (
                <motion.div
                  key={state.current.id}
                  custom={state.lastRating}
                  className="relative w-full"
                  variants={{
                    enter: reduced
                      ? { opacity: 0 }
                      : { opacity: 0, y: 24, scale: 0.96 },
                    show: { opacity: 1, y: 0, scale: 1 },
                    leave: (rating: Rating | null) =>
                      reduced
                        ? { opacity: 0, transition: { duration: 0.1 } }
                        : {
                            opacity: 0,
                            x: (rating && rating >= 3 ? 40 : -40) * side,
                            transition: {
                              duration: 0.14,
                              ease: [0.64, 0, 0.78, 0],
                            },
                          },
                  }}
                  initial="enter"
                  animate="show"
                  exit="leave"
                  transition={
                    reduced
                      ? { duration: 0.1 }
                      : { type: "spring", stiffness: 220, damping: 26 }
                  }
                >
                  <ReviewCardView
                    ws={ws}
                    card={state.current}
                    session={session}
                    keysEnabled={!confirmExit}
                    onRequestExit={requestExit}
                  />
                  {reduced
                    ? null
                    : ([1, 2, 3, 4] as const).map((rating) => (
                        <motion.span
                          key={rating}
                          aria-hidden
                          className={cn(
                            "pointer-events-none absolute inset-0",
                            TINT[rating]
                          )}
                          variants={{
                            leave: (last: Rating | null) => ({
                              opacity: last === rating ? 1 : 0,
                            }),
                          }}
                          initial={{ opacity: 0 }}
                          transition={{ duration: 0.14 }}
                        />
                      ))}
                </motion.div>
              ) : null}
            </AnimatePresence>
          </div>
        </div>
      )}

      <AlertDialog open={confirmExit} onOpenChange={setConfirmExit}>
        <AlertDialogContent data-testid="review-exit-dialog">
          <AlertDialogHeader>
            <AlertDialogTitle>{REVIEW_COPY.exitConfirm}</AlertDialogTitle>
            <AlertDialogDescription>
              {REVIEW_COPY.exitConfirmDesc}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel data-testid="review-exit-cancel">
              {REVIEW_COPY.keepReviewing}
            </AlertDialogCancel>
            <AlertDialogAction
              data-testid="review-exit-confirm"
              onClick={() => {
                setConfirmExit(false)
                onExit()
              }}
            >
              {REVIEW_COPY.exit}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )

  return focus ? (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) {
          if (state.phase === "complete" && !saving) onExit()
          else requestExit()
        }
      }}
    >
      <DialogContent
        showCloseButton={false}
        aria-describedby={undefined}
        onOpenAutoFocus={(event) => {
          event.preventDefault()
          focusRef.current?.focus()
        }}
        onEscapeKeyDown={(event) => {
          event.preventDefault()
          if (!confirmExit) {
            if (state.phase === "complete" && !saving) onExit()
            else requestExit()
          }
        }}
        className="inset-0 start-0 top-0 size-full max-w-none translate-x-0 translate-y-0 overflow-y-auto border-0 bg-background p-0 sm:max-w-none rtl:translate-x-0"
      >
        <DialogTitle className="sr-only">Review session</DialogTitle>
        {content}
      </DialogContent>
    </Dialog>
  ) : (
    content
  )
}
