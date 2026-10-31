import { useCallback, useEffect, useRef, useState } from "react"

const THRESHOLD = 48

/**
 * Keeps a scroll pane pinned to its bottom while content grows, unless the reader scrolled up.
 * `signal` changes whenever content that may grow is rendered (message count, streamed length).
 */
export function useStickToBottom(signal: unknown) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const contentRef = useRef<HTMLDivElement>(null)
  const stuck = useRef(true)
  const [atBottom, setAtBottom] = useState(true)

  const toBottom = useCallback(() => {
    const el = scrollRef.current
    if (el) el.scrollTop = el.scrollHeight
  }, [])

  const onScroll = useCallback(() => {
    const el = scrollRef.current
    if (!el) return
    const near = el.scrollHeight - el.scrollTop - el.clientHeight <= THRESHOLD
    stuck.current = near
    setAtBottom(near)
  }, [])

  useEffect(() => {
    if (stuck.current) toBottom()
  }, [signal, toBottom])

  useEffect(() => {
    const content = contentRef.current
    if (!content) return
    const observer = new ResizeObserver(() => {
      if (stuck.current) toBottom()
    })
    observer.observe(content)
    return () => observer.disconnect()
  }, [toBottom])

  const jumpToLatest = useCallback(() => {
    stuck.current = true
    setAtBottom(true)
    toBottom()
  }, [toBottom])

  return { scrollRef, contentRef, atBottom, onScroll, jumpToLatest }
}
