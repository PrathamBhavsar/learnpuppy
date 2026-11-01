// 3-level chain table: groups (source session) → chains → propositions. Sortable, virtualized,
// keyboard navigable as a tree, and exportable as CSV (docs/web/06, 05 #23).
import { useVirtualizer } from "@tanstack/react-virtual"
import { ArrowDownIcon, ArrowUpIcon, DownloadIcon, XIcon } from "lucide-react"
import { useCallback, useMemo, useRef, useState } from "react"

import { toApiError } from "@/api/errors"
import { useChainTable } from "@/api/queries/graph"
import { InfoTip } from "@/components/app/info/info"
import { SkeletonRows } from "@/components/app/skeleton-block"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

import { copy, formatCount } from "../copy"
import { graphInfo } from "../info"
import {
  type ChainSort,
  DEFAULT_SORT,
  type FlatRow,
  chainsToCsv,
  flattenChains,
  groupRows,
  sortGroups,
} from "../model/chains"
import type { Scene } from "../model/scene"

import { ChainRow } from "./ChainRows"
import { ResizeHandle } from "./ResizeHandle"

const ROW_HEIGHT = 32
const SORTS: { key: ChainSort["key"]; label: string }[] = [
  { key: "group", label: copy.table_.groups },
  { key: "count", label: copy.table_.chains },
  { key: "kind", label: copy.table_.kind },
  { key: "from", label: copy.table_.from },
  { key: "to", label: copy.table_.to },
]

type Props = {
  ws: string
  scene: Scene
  /** Selected node ids scope the table; empty shows every chain. */
  selection: readonly string[]
  height: number
  onResize: (height: number) => void
  onClose: () => void
  /** Row clicked: select these nodes on the canvas. */
  onPickNodes: (ids: string[]) => void
}

function downloadText(name: string, text: string): void {
  if (typeof URL.createObjectURL !== "function") return
  const url = URL.createObjectURL(
    new Blob([text], { type: "text/csv;charset=utf-8" })
  )
  const a = document.createElement("a")
  a.href = url
  a.download = name
  document.body.append(a)
  a.click()
  a.remove()
  URL.revokeObjectURL(url)
}

export function ChainTable({
  ws,
  scene,
  selection,
  height,
  onResize,
  onClose,
  onPickNodes,
}: Props) {
  const query = useChainTable(ws, selection)
  const [sort, setSort] = useState<ChainSort>(DEFAULT_SORT)
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set())
  const [cursor, setCursor] = useState(0)
  const scroller = useRef<HTMLDivElement>(null)

  const textOf = useCallback(
    (id: string) => scene.texts[scene.index.get(id) ?? -1] ?? id,
    [scene]
  )
  const groups = useMemo(
    () => sortGroups(groupRows(query.data ?? []), sort, textOf),
    [query.data, sort, textOf]
  )
  // Open the first group once data arrives so the table never looks empty-but-full.
  const firstGroup = groups[0]?.id
  const [seeded, setSeeded] = useState<string | null>(null)
  if (firstGroup && seeded !== `${ws}:${selection.join(",")}`) {
    setSeeded(`${ws}:${selection.join(",")}`)
    setExpanded(new Set([`g:${firstGroup}`]))
  }
  const rows = useMemo(
    () => flattenChains(groups, expanded),
    [groups, expanded]
  )
  const selectedIds = useMemo(() => new Set(selection), [selection])

  // eslint-disable-next-line react-hooks/incompatible-library -- the compiler is not enabled; the virtualizer is used as documented
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 8,
    getItemKey: (i) => rows[i]?.key ?? i,
  })

  const toggle = useCallback((key: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (!next.delete(key)) next.add(key)
      return next
    })
  }, [])

  const pick = useCallback(
    (row: FlatRow) => {
      if (row.level === 2) onPickNodes([row.row.from, row.row.to])
      else if (row.level === 3) onPickNodes([row.nodeId])
      else toggle(row.key)
    },
    [onPickNodes, toggle]
  )

  const isSelected = (row: FlatRow): boolean =>
    row.level === 3
      ? selectedIds.has(row.nodeId)
      : row.level === 2
        ? selectedIds.has(row.row.from) && selectedIds.has(row.row.to)
        : false

  const move = (to: number) => {
    const next = Math.min(rows.length - 1, Math.max(0, to))
    setCursor(next)
    virtualizer.scrollToIndex(next)
  }
  const onKeyDown = (e: React.KeyboardEvent) => {
    const row = rows[cursor]
    switch (e.key) {
      case "ArrowDown":
        move(cursor + 1)
        break
      case "ArrowUp":
        move(cursor - 1)
        break
      case "Home":
        move(0)
        break
      case "End":
        move(rows.length - 1)
        break
      case "ArrowRight":
      case "ArrowLeft": {
        // Expand/collapse follow the reading direction, like the other keyboard-driven lists.
        const rtl = getComputedStyle(e.currentTarget).direction === "rtl"
        const expand = (e.key === "ArrowRight") !== rtl
        if (row && row.level !== 3 && row.expanded !== expand) toggle(row.key)
        break
      }
      case "Enter":
      case " ":
        if (row) pick(row)
        break
      default:
        return
    }
    e.preventDefault()
  }

  const sortBy = (key: ChainSort["key"]) =>
    setSort((s) => ({
      key,
      dir: s.key === key && s.dir === "asc" ? "desc" : "asc",
    }))
  const active = rows[Math.min(cursor, rows.length - 1)]
  const total = query.data?.length ?? 0

  return (
    <section
      aria-label={copy.table_.label}
      data-testid="graph-table"
      style={{ height }}
      className="relative flex shrink-0 flex-col bg-background"
    >
      <ResizeHandle height={height} onResize={onResize} />
      <header className="flex flex-wrap items-center gap-2 border-b border-border px-3 py-1.5">
        <h2 className="text-[12px] font-medium">{copy.table_.label}</h2>
        <InfoTip info={graphInfo.table} />
        <span
          className="text-[11px] text-muted-foreground tabular-nums"
          data-testid="graph-table-scope"
        >
          {copy.table_.scope(selection.length)} · {formatCount(total)}
        </span>
        <div
          role="group"
          aria-label="Sort"
          className="ms-auto flex items-center"
        >
          {SORTS.map(({ key, label }) => {
            const on = sort.key === key
            return (
              <Button
                key={key}
                variant="ghost"
                size="xs"
                aria-pressed={on}
                aria-label={`Sort by ${label}${on ? `, ${sort.dir === "asc" ? "ascending" : "descending"}` : ""}`}
                data-testid={`graph-table-sort-${key}`}
                onClick={() => sortBy(key)}
                className={cn(
                  "text-[11px]",
                  on ? "text-foreground" : "text-muted-foreground"
                )}
              >
                {label}
                {on ? (
                  sort.dir === "asc" ? (
                    <ArrowUpIcon data-icon="inline-end" />
                  ) : (
                    <ArrowDownIcon data-icon="inline-end" />
                  )
                ) : null}
              </Button>
            )
          })}
        </div>
        <Button
          variant="outline"
          size="xs"
          disabled={groups.length === 0}
          onClick={() =>
            downloadText(copy.table_.csvName, chainsToCsv(groups, textOf))
          }
          data-testid="graph-table-export"
        >
          <DownloadIcon data-icon="inline-start" />
          {copy.table_.export}
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Close chain table"
          onClick={onClose}
          data-testid="graph-table-close"
        >
          <XIcon />
        </Button>
      </header>
      {query.isPending ? (
        <div className="p-3">
          <SkeletonRows rows={4} />
        </div>
      ) : query.isError ? (
        <div role="alert" className="flex items-center gap-3 p-3 text-[12px]">
          <span className="text-destructive">
            {copy.table_.loadFailed}: {toApiError(query.error).message}
          </span>
          <Button
            variant="outline"
            size="xs"
            onClick={() => void query.refetch()}
          >
            Retry
          </Button>
        </div>
      ) : rows.length === 0 ? (
        <p role="status" className="p-4 text-[12px] text-muted-foreground">
          {selection.length ? copy.table_.emptySelection : copy.table_.empty}
        </p>
      ) : (
        <div
          ref={scroller}
          role="tree"
          tabIndex={0}
          aria-label={copy.table_.label}
          aria-activedescendant={
            active ? `graph-chain-row-${active.key}` : undefined
          }
          onKeyDown={onKeyDown}
          data-testid="graph-table-tree"
          className="lp-pane min-h-0 flex-1 overflow-y-auto outline-none focus-visible:ring-1 focus-visible:ring-ring/50 focus-visible:ring-inset"
        >
          <div
            style={{ height: virtualizer.getTotalSize(), position: "relative" }}
          >
            {virtualizer.getVirtualItems().map((item) => {
              const row = rows[item.index]
              if (!row) return null
              return (
                <div
                  key={item.key}
                  style={{
                    position: "absolute",
                    insetInlineStart: 0,
                    insetInlineEnd: 0,
                    top: 0,
                    height: item.size,
                    transform: `translateY(${item.start}px)`,
                  }}
                >
                  <ChainRow
                    row={row}
                    position={item.index + 1}
                    size={rows.length}
                    active={item.index === cursor}
                    selected={isSelected(row)}
                    textOf={textOf}
                    onToggle={toggle}
                    onPick={(r) => {
                      setCursor(item.index)
                      pick(r)
                    }}
                  />
                </div>
              )
            })}
          </div>
        </div>
      )}
    </section>
  )
}
