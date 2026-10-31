import { useState } from "react"

import { useGraph } from "@/api/queries/graph"
import { SkeletonRows } from "@/components/app/skeleton-block"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"

type NodePickerProps = {
  ws: string
  selected: readonly string[]
  onChange: (ids: string[]) => void
}

const LIMIT = 40

/** Searchable list of graph propositions to seed a chat with. */
export function NodePicker({ ws, selected, onChange }: NodePickerProps) {
  const graph = useGraph(ws)
  const [query, setQuery] = useState("")
  if (graph.isPending) return <SkeletonRows rows={3} />
  if (graph.isError)
    return (
      <p className="text-[12px] text-destructive">Could not load the graph.</p>
    )
  const needle = query.trim().toLowerCase()
  const nodes = graph.data.nodes
    .filter((n) => needle === "" || n.text.toLowerCase().includes(needle))
    .slice(0, LIMIT)
  const toggle = (id: string) =>
    onChange(
      selected.includes(id)
        ? selected.filter((s) => s !== id)
        : [...selected, id]
    )
  return (
    <div className="flex flex-col gap-2" data-testid="node-picker">
      <Input
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        placeholder="Search the graph"
        aria-label="Search graph nodes"
        data-testid="node-picker-search"
      />
      {nodes.length === 0 ? (
        <p className="text-[12px] text-muted-foreground">
          {graph.data.nodes.length === 0
            ? "This workspace has no knowledge yet."
            : "Nothing matches."}
        </p>
      ) : (
        <ul
          className="max-h-40 overflow-y-auto border"
          aria-label="Graph nodes"
        >
          {nodes.map((node) => (
            <li key={node.id}>
              <label className="flex cursor-pointer items-start gap-2 px-2 py-1.5 text-[12px] hover:bg-muted/50">
                <Checkbox
                  checked={selected.includes(node.id)}
                  onCheckedChange={() => toggle(node.id)}
                  data-testid={`node-pick-${node.id}`}
                  className="mt-0.5"
                />
                <span dir="auto" className="min-w-0 flex-1">
                  {node.text}
                </span>
              </label>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
