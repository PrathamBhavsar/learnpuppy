import { GraduationCapIcon, Trash2Icon, XIcon } from "lucide-react"
import { motion } from "motion/react"

import { InfoTip } from "@/components/app/info/info"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { spring } from "@/lib/motion"

import { copy } from "../copy"
import { graphInfo } from "../info"

type Props = {
  count: number
  onPractice: () => void
  onDelete: () => void
  onClear: () => void
  practicePending: boolean
  /** Width in px the NodeDrawer covers at the inline end; the bar centres itself in what is left. */
  dockInset: number
}

/** Springs up from the bottom edge whenever at least one proposition is selected (docs/web/05 #20). */
export function SelectionBar({
  count,
  onPractice,
  onDelete,
  onClear,
  practicePending,
  dockInset,
}: Props) {
  const reduced = useReducedMotion()
  return (
    <motion.div
      role="region"
      aria-label="Selection"
      data-testid="graph-selection-bar"
      initial={{ y: 56, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      exit={{ y: 56, opacity: 0 }}
      transition={spring.gentle}
      // Glides sideways (a transform) when the drawer opens or closes and the free area changes.
      layout={reduced ? false : "position"}
      // Both insets plus auto margins centre a fit-content box in the free area, in either direction.
      style={{
        insetInlineStart: 0,
        insetInlineEnd: dockInset,
        marginInline: "auto",
        width: "fit-content",
      }}
      className="pointer-events-auto absolute bottom-4 z-20 flex items-center gap-2 border border-border bg-background px-3 py-2 shadow-sm"
    >
      <span
        aria-live="polite"
        data-testid="graph-selection-count"
        className="pe-2 text-[12px] font-medium tabular-nums"
      >
        {copy.selection.count(count)}
      </span>
      <InfoTip info={graphInfo.selection} />
      <Button
        size="sm"
        onClick={onPractice}
        disabled={practicePending}
        data-testid="graph-selection-practice"
      >
        {practicePending ? (
          <Spinner data-icon="inline-start" />
        ) : (
          <GraduationCapIcon data-icon="inline-start" />
        )}
        {copy.selection.practice}
      </Button>
      <Button
        variant="destructive"
        size="sm"
        onClick={onDelete}
        data-testid="graph-selection-delete"
      >
        <Trash2Icon data-icon="inline-start" />
        {copy.selection.delete}
      </Button>
      <Button
        variant="ghost"
        size="sm"
        onClick={onClear}
        data-testid="graph-selection-clear"
      >
        <XIcon data-icon="inline-start" />
        {copy.selection.clear}
      </Button>
    </motion.div>
  )
}
