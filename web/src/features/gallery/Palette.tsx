import { HUE_NAMES } from "@/features/gallery/hues"

import { Section, Specimen } from "./Section"

const GROUPS: { label: string; tokens: string[] }[] = [
  {
    label: "Surfaces",
    tokens: ["background", "card", "popover", "secondary", "muted", "sidebar"],
  },
  {
    label: "Actions and status",
    tokens: [
      "primary",
      "accent",
      "destructive",
      "success",
      "warning",
      "info",
      "ring",
    ],
  },
  {
    label: "Mastery",
    tokens: [0, 1, 2, 3, 4].map((i) => `mastery-${i}`),
  },
  {
    label: "Sources",
    tokens: ["source-claude", "source-codex", "source-agy", "source-git"],
  },
  {
    label: "Clusters (workspace hues)",
    tokens: HUE_NAMES.map((_, i) => `cluster-${i + 1}`),
  },
  { label: "Chart", tokens: [1, 2, 3, 4, 5].map((i) => `chart-${i}`) },
]

export function Palette() {
  return (
    <Section id="palette" title="Palette tokens (from tokens.css)">
      {GROUPS.map((group) => (
        <Specimen key={group.label} label={group.label} className="max-w-md">
          {group.tokens.map((token) => (
            <div key={token} className="flex w-24 flex-col gap-1">
              <span
                className="h-8 border"
                style={{ background: `var(--${token})` }}
                data-testid={`swatch-${token}`}
              />
              <span className="truncate text-[11px] text-muted-foreground">
                {token}
              </span>
            </div>
          ))}
        </Specimen>
      ))}
    </Section>
  )
}
