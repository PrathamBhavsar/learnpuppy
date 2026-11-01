import { SearchIcon } from "lucide-react"

import type { NodeSource } from "@/api/pending-types"
import { InfoTip } from "@/components/app/info/info"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { cn } from "@/lib/utils"

import { DIGEST_COPY } from "./copy"
import { digestInfo } from "./info"
import { RANGE_PRESETS, type RangePreset, isRangePreset } from "./range"
import { SOURCE_LABELS, SourceIcon } from "./source-icon"

export const DIGEST_SOURCES = [
  "claude",
  "codex",
  "agy",
] as const satisfies readonly NodeSource[]

type Props = {
  range: RangePreset
  from: string | undefined
  to: string | undefined
  sources: NodeSource[]
  query: string
  onRange: (range: RangePreset) => void
  onCustom: (next: { from?: string; to?: string }) => void
  onSources: (sources: NodeSource[]) => void
  onQuery: (query: string) => void
}

export function DigestControls({
  range,
  from,
  to,
  sources,
  query,
  onRange,
  onCustom,
  onSources,
  onQuery,
}: Props) {
  const toggleSource = (source: NodeSource) =>
    onSources(
      sources.includes(source)
        ? sources.filter((s) => s !== source)
        : [...sources, source]
    )

  return (
    <div data-testid="digest-controls" className="flex flex-col gap-3">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-center">
        <ToggleGroup
          type="single"
          value={range}
          onValueChange={(v) => isRangePreset(v) && onRange(v)}
          variant="outline"
          aria-label={DIGEST_COPY.rangeLabel}
          data-testid="digest-range"
        >
          {RANGE_PRESETS.map((preset) => (
            <ToggleGroupItem
              dir="auto"
              key={preset}
              value={preset}
              data-testid={`digest-range-${preset}`}
              className="text-[13px]"
            >
              {DIGEST_COPY.presets[preset]}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
        <InfoTip info={digestInfo.range} />
        {range === "custom" ? (
          <div
            className="flex items-center gap-2"
            data-testid="digest-custom-range"
          >
            <Label
              htmlFor="digest-from"
              className="text-[12px] text-muted-foreground"
            >
              {DIGEST_COPY.from}
            </Label>
            <Input
              id="digest-from"
              type="date"
              value={from ?? ""}
              max={to}
              onChange={(e) =>
                onCustom({ from: e.target.value || undefined, to })
              }
              data-testid="digest-from"
              className="w-40"
            />
            <Label
              htmlFor="digest-to"
              className="text-[12px] text-muted-foreground"
            >
              {DIGEST_COPY.to}
            </Label>
            <Input
              id="digest-to"
              type="date"
              value={to ?? ""}
              min={from}
              onChange={(e) =>
                onCustom({ from, to: e.target.value || undefined })
              }
              data-testid="digest-to"
              className="w-40"
            />
          </div>
        ) : null}
        <div className="relative flex-1">
          <SearchIcon
            aria-hidden
            className="pointer-events-none absolute start-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            type="search"
            value={query}
            onChange={(e) => onQuery(e.target.value)}
            placeholder={DIGEST_COPY.searchPlaceholder}
            aria-label={DIGEST_COPY.searchLabel}
            data-testid="digest-search"
            data-screen-search
            className="ps-8"
          />
        </div>
        <InfoTip info={digestInfo.search} />
      </div>
      <div
        role="group"
        aria-label={DIGEST_COPY.sourcesLabel}
        data-testid="digest-sources"
        className="flex flex-wrap items-center gap-2"
      >
        {DIGEST_SOURCES.map((source) => {
          const on = sources.includes(source)
          return (
            <button
              key={source}
              type="button"
              aria-pressed={on}
              onClick={() => toggleSource(source)}
              data-testid={`digest-source-${source}`}
              className={cn(
                "inline-flex h-7 items-center gap-1.5 border px-2.5 text-[12px] transition-colors outline-none focus-visible:ring-1 focus-visible:ring-ring",
                on
                  ? "border-primary bg-primary/10 text-foreground"
                  : "text-muted-foreground hover:bg-accent/50"
              )}
            >
              <SourceIcon source={source} className="size-3.5" />
              {SOURCE_LABELS[source]}
            </button>
          )
        })}
        <InfoTip info={digestInfo.sources} />
        <InfoTip info={digestInfo.sessions} />
      </div>
    </div>
  )
}
