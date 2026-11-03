import { motion } from "motion/react"
import { CheckIcon, XIcon } from "lucide-react"
import { type ButtonHTMLAttributes, forwardRef } from "react"

import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/app/kbd"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { cn } from "@/lib/utils"

import type { ReviewCard } from "@/api/pending-types"
import { LazyMarkdown } from "@/components/app/markdown/LazyMarkdown"

const KEYS = ["a", "b", "c", "d"] as const
const NUM_KEYS = ["1", "2", "3", "4"] as const

type ChoicesProps = {
  card: ReviewCard
  /** Disabled while the answer is being checked. */
  pending: boolean
  /** User's chosen key (when answered). */
  chosen: string | null
  /** Reveal state: correct key + whether the chosen was correct. */
  correctKey: string | null
  answered: boolean
  onSelect: (key: string) => void
}

export const Choices = forwardRef<HTMLDivElement, ChoicesProps>(
  function Choices(
    { card, pending, chosen, correctKey, answered, onSelect },
    ref
  ) {
    const reduced = useReducedMotion()
    const options = card.options ?? []

    return (
      <div
        ref={ref}
        data-testid="review-choices"
        className="grid gap-2"
        role="group"
        aria-label="Answer choices"
      >
        {options.map((opt, i) => {
          const key = opt.key
          const isChosen = chosen === key
          const isCorrect = answered && key === correctKey
          const isWrongChosen = answered && isChosen && !isCorrect
          return (
            <motion.div
              key={key}
              data-correct={isCorrect ? "" : undefined}
              data-wrong={isWrongChosen ? "" : undefined}
              initial={reduced ? false : { opacity: 0, y: 8 }}
              animate={
                isWrongChosen && !reduced
                  ? { opacity: 1, y: 0, x: [0, -4, 4, -4, 4, -4, 4, 0] }
                  : { opacity: 1, y: 0, x: 0 }
              }
              transition={
                isWrongChosen && !reduced
                  ? { x: { duration: 0.24 } }
                  : { duration: 0.14, delay: Math.min(i, 7) * 0.024 }
              }
            >
              <ChoiceButton
                data-testid={`review-choice-${key.toLowerCase()}`}
                disabled={pending || answered}
                pending={pending && isChosen}
                highlighted={isCorrect}
                wrong={isWrongChosen}
                onClick={() => onSelect(key)}
                aria-pressed={isChosen}
              >
                <span className="flex min-w-0 items-center gap-2.5 text-start">
                  <span
                    aria-hidden
                    className="flex size-5 shrink-0 items-center justify-center border text-[11px] font-medium tabular-nums"
                  >
                    {key}
                  </span>
                  <LazyMarkdown className="min-w-0 flex-1 text-[13px]">
                    {opt.text}
                  </LazyMarkdown>
                  {isCorrect ? (
                    <CheckIcon className="size-4 shrink-0 text-success" />
                  ) : isWrongChosen ? (
                    <XIcon className="size-4 shrink-0 text-destructive" />
                  ) : null}
                </span>
                <span className="flex shrink-0 items-center gap-1 text-[10px] text-muted-foreground">
                  <Kbd combo={NUM_KEYS[i]!} className="hidden sm:inline-flex" />
                  <Kbd combo={KEYS[i]!} className="hidden sm:inline-flex" />
                </span>
              </ChoiceButton>
            </motion.div>
          )
        })}
      </div>
    )
  }
)

type ChoiceButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  pending?: boolean
  highlighted?: boolean
  wrong?: boolean
}

const ChoiceButton = forwardRef<HTMLButtonElement, ChoiceButtonProps>(
  function ChoiceButton(
    { pending, highlighted, wrong, className, children, ...props },
    ref
  ) {
    return (
      <Button
        ref={ref}
        type="button"
        variant="outline"
        data-state={
          highlighted
            ? "correct"
            : wrong
              ? "wrong"
              : pending
                ? "pending"
                : "default"
        }
        className={cn(
          "h-auto w-full justify-between py-2.5 text-[13px] font-normal whitespace-normal",
          highlighted && "border-success bg-success/10 text-success",
          wrong && "border-destructive bg-destructive/10 text-destructive",
          pending && "opacity-60",
          className
        )}
        {...props}
      >
        {children}
      </Button>
    )
  }
)
