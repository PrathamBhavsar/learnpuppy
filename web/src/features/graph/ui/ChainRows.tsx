/* eslint-disable jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- rows are treeitems driven by the tree container (aria-activedescendant); the click handlers are the pointer path to the same actions */
import { ArrowRightIcon, ChevronRightIcon } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

import { copy, formatCount } from "../copy"
import type { FlatRow } from "../model/chains"

type Props = {
  row: FlatRow
  position: number
  size: number
  active: boolean
  selected: boolean
  textOf: (id: string) => string
  onToggle: (key: string) => void
  onPick: (row: FlatRow) => void
}

const INDENT = [0, 0, 20, 40] as const

function Chevron({ open }: { open: boolean }) {
  return (
    <ChevronRightIcon
      aria-hidden
      className={cn(
        "size-3.5 shrink-0 text-muted-foreground transition-transform duration-[var(--dur-fast)] rtl:-scale-x-100",
        open && "rotate-90 rtl:rotate-90"
      )}
    />
  )
}

/** One virtualized row of the 3-level chain table (group → chain → proposition). */
export function ChainRow({
  row,
  position,
  size,
  active,
  selected,
  textOf,
  onToggle,
  onPick,
}: Props) {
  const common = {
    role: "treeitem" as const,
    id: `graph-chain-row-${row.key}`,
    "aria-level": row.level,
    "aria-posinset": position,
    "aria-setsize": size,
    "aria-selected": selected,
    "data-testid": `graph-chain-row-l${row.level}`,
    style: { paddingInlineStart: 8 + INDENT[row.level] },
    className: cn(
      "flex h-full animate-in items-center gap-2 pe-3 text-[12px] duration-150 fade-in-0 slide-in-from-top-1",
      "cursor-pointer border-b border-border/50 hover:bg-muted/60",
      selected && "bg-primary/10",
      active && "outline outline-1 -outline-offset-1 outline-ring"
    ),
  }

  if (row.level === 1) {
    return (
      <div
        {...common}
        aria-expanded={row.expanded}
        onClick={() => onToggle(row.key)}
      >
        <Chevron open={row.expanded} />
        <span
          className="min-w-0 flex-1 truncate font-medium"
          title={row.group.label}
        >
          {row.group.label}
        </span>
        <span className="text-muted-foreground tabular-nums">
          {formatCount(row.group.rows.length)}
        </span>
      </div>
    )
  }
  if (row.level === 2) {
    const from = textOf(row.row.from)
    const to = textOf(row.row.to)
    return (
      <div {...common} aria-expanded={row.expanded} onClick={() => onPick(row)}>
        <button
          type="button"
          tabIndex={-1}
          aria-label={row.expanded ? copy.table_.collapse : copy.table_.expand}
          className="-ms-1 p-1 outline-none"
          onClick={(e) => {
            e.stopPropagation()
            onToggle(row.key)
          }}
        >
          <Chevron open={row.expanded} />
        </button>
        <Badge variant="outline" className="shrink-0">
          {copy.drawer.chainKinds[row.row.kind] ?? row.row.kind}
        </Badge>
        <span
          dir="auto"
          className="min-w-0 flex-1 truncate"
          title={`${from} → ${to}`}
        >
          {from}
        </span>
        <ArrowRightIcon
          aria-hidden
          className="size-3 shrink-0 text-muted-foreground rtl:-scale-x-100"
        />
        <span dir="auto" className="min-w-0 flex-1 truncate">
          {to}
        </span>
      </div>
    )
  }
  return (
    <div {...common} onClick={() => onPick(row)}>
      <Badge variant="secondary" className="shrink-0">
        {row.role === "from" ? copy.table_.premise : copy.table_.conclusion}
      </Badge>
      <span
        dir="auto"
        className="min-w-0 flex-1 truncate"
        title={textOf(row.nodeId)}
      >
        {textOf(row.nodeId)}
      </span>
    </div>
  )
}
