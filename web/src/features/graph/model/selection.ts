// Selection reducers. `selected` keeps insertion order (the last entry is the most recent pick) and
// `active` is the node the drawer shows. Pure, so the interaction rules are unit tested directly.

export type Selection = { selected: readonly string[]; active: string | null }

export const EMPTY_SELECTION: Selection = { selected: [], active: null }

export function selectOnly(id: string): Selection {
  return { selected: [id], active: id }
}

/** Shift/⌘-click: add the node, or remove it if already selected. */
export function toggleSelected(sel: Selection, id: string): Selection {
  if (sel.selected.includes(id)) {
    const selected = sel.selected.filter((s) => s !== id)
    const active =
      sel.active === id ? (selected[selected.length - 1] ?? null) : sel.active
    return { selected, active }
  }
  return { selected: [...sel.selected, id], active: id }
}

/**
 * Marquee result. Shift keeps the existing selection and adds the swept nodes; without it the sweep
 * replaces the selection. A single swept node becomes active, several leave the drawer closed.
 */
export function selectMany(
  sel: Selection,
  ids: readonly string[],
  additive: boolean
): Selection {
  const merged = additive ? [...sel.selected] : []
  const have = new Set(merged)
  for (const id of ids)
    if (!have.has(id)) {
      have.add(id)
      merged.push(id)
    }
  if (merged.length === 0) return EMPTY_SELECTION
  const active =
    merged.length === 1
      ? (merged[0] as string)
      : sel.active && have.has(sel.active) && additive
        ? sel.active
        : null
  return { selected: merged, active }
}

/** Drops ids that no longer exist (after a delete or a refetch). */
export function pruneSelection(sel: Selection, exists: (id: string) => boolean): Selection {
  const selected = sel.selected.filter(exists)
  if (selected.length === sel.selected.length) return sel
  const active = sel.active && exists(sel.active) ? sel.active : (selected[selected.length - 1] ?? null)
  return { selected, active }
}
