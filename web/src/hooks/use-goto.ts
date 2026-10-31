import { useRouter } from "@tanstack/react-router"
import { useCallback } from "react"

/** Navigate to an already-built path (hotkeys and palette commands compute theirs at runtime). */
export function useGoto(): (path: string) => void {
  const router = useRouter()
  return useCallback((path) => router.history.push(path), [router])
}
