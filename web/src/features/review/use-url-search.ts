import { useEffect, useState } from "react"
import { useDebounced } from "@/hooks/use-debounced"

/** Keep the editable search in sync with both typing and Back/Forward navigation. */
export function useUrlSearch(query: string, onChange: (query: string) => void) {
  const [previous, setPrevious] = useState(query)
  const [draft, setDraft] = useState(query)
  if (previous !== query) {
    setPrevious(query)
    setDraft(query)
  }
  const debounced = useDebounced(draft, 250)
  useEffect(() => {
    if (debounced === draft && draft !== query) onChange(draft)
  }, [debounced, draft, query, onChange])
  return [draft, setDraft] as const
}
