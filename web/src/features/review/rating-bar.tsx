import { motion } from "motion/react"
import { type ButtonHTMLAttributes, forwardRef } from "react"

import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/app/kbd"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { cn } from "@/lib/utils"

import type { IntervalPreview, Rating } from "@/api/pending-types"
import type { components } from "@/api/schema"
import { REVIEW_COPY } from "./copy"
import { formatIntervalDays } from "./time"

const LABELS = REVIEW_COPY.ratingLabels
const RATING_COLORS: Record<Rating, string> = {
  1: "text-destructive",
  2: "text-warning",
  3: "text-info",
  4: "text-success",
}

type RatingBarProps = {
  preview: IntervalPreview
  items?: components["schemas"]["PreviewDto"][]
  /** Server-suggested rating (default highlight). */
  suggestion: Rating
  /** Rating whose key was just pressed, so the keyboard feels physical. */
  pressed: Rating | null
  onRate: (rating: Rating) => void
}

const KEYS = ["1", "2", "3", "4"] as const
const RATINGS: Rating[] = [1, 2, 3, 4]

export const RatingBar = forwardRef<HTMLDivElement, RatingBarProps>(
  function RatingBar({ preview, items, suggestion, pressed, onRate }, ref) {
    const reduced = useReducedMotion()
    const values: Record<Rating, number> = {
      1: preview.again,
      2: preview.hard,
      3: preview.good,
      4: preview.easy,
    }

    return (
      <div
        ref={ref}
        data-testid="review-rating-bar"
        className="grid grid-cols-2 gap-2 sm:grid-cols-4"
        role="group"
        aria-label="Rate your recall"
      >
        {RATINGS.map((rating, i) => {
          const isSuggestion = rating === suggestion
          return (
            <motion.div
              key={rating}
              data-suggested={isSuggestion ? "" : undefined}
              initial={reduced ? false : { opacity: 0, y: 8 }}
              animate={{
                opacity: 1,
                y: 0,
                scale: pressed === rating ? 0.97 : 1,
              }}
              transition={{
                duration: 0.08,
                delay: pressed ? 0 : Math.min(i, 7) * 0.024,
              }}
            >
              <RatingButton
                data-testid={`review-rating-${rating}`}
                suggested={isSuggestion}
                onClick={() => onRate(rating)}
              >
                <span className="flex flex-col items-center gap-0.5">
                  <span className="font-heading text-[14px] font-medium">
                    {LABELS[rating - 1]}
                  </span>
                  <span
                    className={cn(
                      "text-[11px] tabular-nums",
                      RATING_COLORS[rating]
                    )}
                  >
                    {items?.find((p) => p.rating === rating)?.intervalLabel ??
                      formatIntervalDays(values[rating])}
                  </span>
                </span>
                <span className="flex items-center gap-1">
                  {isSuggestion ? (
                    <span
                      className="text-[10px] text-muted-foreground"
                      title="Suggested. Press Space to accept."
                    >
                      Space
                    </span>
                  ) : null}
                  <Kbd combo={KEYS[i]!} className="hidden sm:inline-flex" />
                </span>
              </RatingButton>
            </motion.div>
          )
        })}
      </div>
    )
  }
)

type RatingButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  suggested?: boolean
}

const RatingButton = forwardRef<HTMLButtonElement, RatingButtonProps>(
  function RatingButton({ suggested, className, children, ...props }, ref) {
    return (
      <Button
        ref={ref}
        type="button"
        variant="outline"
        className={cn(
          "h-auto w-full flex-col items-stretch justify-between gap-1 py-2 text-[13px]",
          suggested && "border-primary ring-1 ring-primary",
          className
        )}
        {...props}
      >
        {children}
      </Button>
    )
  }
)
