import {
  EyeIcon,
  MapIcon,
  NavigationIcon,
  SearchIcon,
  SlidersHorizontalIcon,
  Table2Icon,
  TagsIcon,
  XIcon,
  HexagonIcon,
} from "lucide-react"
import { useEffect, useRef, useState } from "react"

import type { NodeSource } from "@/api/pending-types"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Slider } from "@/components/ui/slider"
import { Toggle } from "@/components/ui/toggle"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { useDebounced } from "@/hooks/use-debounced"
import { cn } from "@/lib/utils"
import type { LabelMode } from "@/stores/graphView"

import { copy } from "../copy"
import { SOURCES } from "../model/scene"

import { SOURCE_BG } from "./source-style"

const ALL = "__all__"
const SEARCH_DEBOUNCE_MS = 250

export type ToolbarProps = {
  query: string
  onQuery: (q: string) => void
  matchCount: number | null
  searching: boolean
  onSubmitSearch: () => void
  sources: readonly NodeSource[]
  onSources: (s: NodeSource[]) => void
  clusters: readonly { id: string; label: string }[]
  cluster: string | null
  onCluster: (id: string | null) => void
  minDegree: number
  maxDegree: number
  onMinDegree: (n: number) => void
  filtersActive: boolean
  onClearFilters: () => void
  hulls: boolean
  onHulls: (v: boolean) => void
  labels: LabelMode
  onLabels: (v: LabelMode) => void
  minimap: boolean
  onMinimap: (v: boolean) => void
  followNew: boolean
  onFollowNew: (v: boolean) => void
  table: boolean
  onTable: (v: boolean) => void
  counts: string
  /** Width in px the NodeDrawer covers at the inline end; the toolbar wraps inside what is left. */
  dockInset: number
}

function SearchBox(
  props: Pick<
    ToolbarProps,
    "query" | "onQuery" | "matchCount" | "searching" | "onSubmitSearch"
  >
) {
  const { query, onQuery, matchCount, searching, onSubmitSearch } = props
  const [draft, setDraft] = useState(query)
  const lastSent = useRef(query)
  const debounced = useDebounced(draft, SEARCH_DEBOUNCE_MS)

  useEffect(() => {
    if (debounced === lastSent.current) return
    lastSent.current = debounced
    onQuery(debounced)
  }, [debounced, onQuery])

  // Adopt a query that arrived from outside (palette link, Back/Forward) but never clobber typing.
  useEffect(() => {
    if (query !== lastSent.current) {
      lastSent.current = query
      setDraft(query)
    }
  }, [query])

  return (
    <InputGroup className="pointer-events-auto w-60 bg-background">
      <InputGroupAddon>
        <SearchIcon aria-hidden />
      </InputGroupAddon>
      <InputGroupInput
        type="search"
        role="searchbox"
        data-screen-search
        data-testid="graph-search"
        aria-label={copy.search}
        placeholder={copy.searchPlaceholder}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault()
            lastSent.current = draft
            onQuery(draft)
            onSubmitSearch()
          } else if (e.key === "Escape" && draft) {
            e.preventDefault()
            e.stopPropagation()
            setDraft("")
          }
        }}
        className="[&::-webkit-search-cancel-button]:hidden"
      />
      {draft ? (
        <InputGroupAddon align="inline-end">
          {searching && matchCount !== null ? (
            <span
              data-testid="graph-search-count"
              className="text-[11px] tabular-nums"
            >
              {copy.searchMatches(matchCount)}
            </span>
          ) : null}
          <InputGroupButton
            size="icon-xs"
            aria-label={copy.searchClear}
            data-testid="graph-search-clear"
            onClick={() => setDraft("")}
          >
            <XIcon />
          </InputGroupButton>
        </InputGroupAddon>
      ) : null}
    </InputGroup>
  )
}

function Filters(props: ToolbarProps) {
  const {
    sources,
    onSources,
    clusters,
    cluster,
    onCluster,
    minDegree,
    maxDegree,
    onMinDegree,
  } = props
  return (
    <>
      <ToggleGroup
        type="multiple"
        variant="outline"
        size="sm"
        spacing={0}
        aria-label={copy.sources}
        value={[...sources]}
        onValueChange={(v) => onSources(SOURCES.filter((s) => v.includes(s)))}
        className="pointer-events-auto bg-background"
        data-testid="graph-filter-sources"
      >
        {SOURCES.map((source) => (
          <ToggleGroupItem
            key={source}
            value={source}
            aria-label={copy.legend.sourceNames[source]}
            data-testid={`graph-filter-source-${source}`}
            className="gap-1.5 px-2"
          >
            <span aria-hidden className={cn("size-2", SOURCE_BG[source])} />
            {copy.legend.sourceNames[source]}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
      <Select
        value={cluster ?? ALL}
        onValueChange={(v) => onCluster(v === ALL ? null : v)}
      >
        <SelectTrigger
          size="sm"
          aria-label={copy.cluster}
          data-testid="graph-filter-cluster"
          className="pointer-events-auto w-40 bg-background"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value={ALL}>{copy.allClusters}</SelectItem>
          {clusters.map((c) => (
            <SelectItem key={c.id} value={c.id}>
              {c.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Popover>
        <PopoverTrigger asChild>
          <Button
            variant="outline"
            size="sm"
            data-testid="graph-filter-degree"
            aria-label={`${copy.minDegree}: ${minDegree}`}
            className="pointer-events-auto bg-background tabular-nums"
          >
            <SlidersHorizontalIcon data-icon="inline-start" />
            {`≥ ${minDegree}`}
          </Button>
        </PopoverTrigger>
        <PopoverContent align="start" className="w-56 gap-3">
          <label htmlFor="graph-min-degree" className="text-[12px] font-medium">
            {copy.minDegree}
          </label>
          <Slider
            id="graph-min-degree"
            aria-label={copy.minDegree}
            min={0}
            max={Math.max(maxDegree, 1)}
            step={1}
            value={[minDegree]}
            onValueChange={([v]) => onMinDegree(v ?? 0)}
          />
          <p className="text-[11px] text-muted-foreground tabular-nums">{`${minDegree} – ${maxDegree}`}</p>
        </PopoverContent>
      </Popover>
    </>
  )
}

function ViewToggles(props: ToolbarProps) {
  return (
    <>
      <Toggle
        variant="outline"
        size="sm"
        pressed={props.hulls}
        onPressedChange={props.onHulls}
        aria-label={copy.hulls}
        data-testid="graph-toggle-hulls"
        className="pointer-events-auto bg-background"
      >
        <HexagonIcon />
      </Toggle>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="outline"
            size="sm"
            aria-label={`${copy.labels}: ${props.labels}`}
            data-testid="graph-labels-menu"
            className="pointer-events-auto bg-background"
          >
            <TagsIcon />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuLabel>{copy.labels}</DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuRadioGroup
            value={props.labels}
            onValueChange={(v) => props.onLabels(v as LabelMode)}
          >
            <DropdownMenuRadioItem value="auto" data-testid="graph-labels-auto">
              {copy.labelsAuto}
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem
              value="always"
              data-testid="graph-labels-always"
            >
              {copy.labelsAlways}
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem
              value="never"
              data-testid="graph-labels-never"
            >
              {copy.labelsNever}
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
      <Toggle
        variant="outline"
        size="sm"
        pressed={props.minimap}
        onPressedChange={props.onMinimap}
        aria-label={copy.minimap}
        data-testid="graph-toggle-minimap"
        className="pointer-events-auto bg-background"
      >
        <MapIcon />
      </Toggle>
      <Toggle
        variant="outline"
        size="sm"
        pressed={props.followNew}
        onPressedChange={props.onFollowNew}
        aria-label={copy.followNew}
        data-testid="graph-toggle-follow"
        className="pointer-events-auto bg-background"
      >
        <NavigationIcon />
      </Toggle>
      <Toggle
        variant="outline"
        size="sm"
        pressed={props.table}
        onPressedChange={props.onTable}
        aria-label={copy.table}
        data-testid="graph-toggle-table"
        className="pointer-events-auto gap-1.5 bg-background"
      >
        <Table2Icon data-icon="inline-start" />
        Table
      </Toggle>
    </>
  )
}

/** Search, filters, view toggles and counts, floating at the top of the canvas. */
export function Toolbar(props: ToolbarProps) {
  return (
    <div
      role="toolbar"
      aria-label="Graph tools"
      data-testid="graph-toolbar"
      style={{ insetInlineEnd: 12 + props.dockInset }}
      className="pointer-events-none absolute start-3 top-3 z-20 flex flex-wrap items-start justify-between gap-2"
    >
      <div className="flex flex-wrap items-center gap-2">
        <SearchBox {...props} />
        <Filters {...props} />
        {props.filtersActive ? (
          <Button
            variant="ghost"
            size="sm"
            onClick={props.onClearFilters}
            data-testid="graph-clear-filters"
            className="pointer-events-auto bg-background/80"
          >
            <EyeIcon data-icon="inline-start" />
            {copy.clearFilters}
          </Button>
        ) : null}
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <span
          data-testid="graph-counts"
          className="pointer-events-auto border border-border bg-background px-2 py-1 text-[11px] text-muted-foreground tabular-nums"
        >
          {props.counts}
        </span>
        <ViewToggles {...props} />
      </div>
    </div>
  )
}
