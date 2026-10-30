import { useEffect } from "react"

/** `document.title = "<Section> · <Workspace> · LearnPuppy"` (parts joined, falsy parts skipped). */
export function usePageTitle(...parts: (string | undefined)[]): void {
  const title = [...parts.filter(Boolean), "LearnPuppy"].join(" · ")
  useEffect(() => {
    document.title = title
  }, [title])
}
