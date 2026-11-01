import { createContext, useContext, useState, type ReactNode } from "react"

import { storage } from "@/lib/storage"

const Scope = createContext<string | null>(null)

export function DraftScope({
  ws,
  chatId,
  children,
}: {
  ws: string
  chatId: string
  children: ReactNode
}) {
  return <Scope value={JSON.stringify([ws, chatId])}>{children}</Scope>
}

/** Write on each edit so navigation/reload cannot race an effect or debounce. */
export function useDraft(question?: string) {
  const scope = useContext(Scope)
  const key =
    scope === null
      ? null
      : `lp-draft:${scope}:${question === undefined ? "composer" : `answer:${question}`}`
  const [state, setState] = useState(() => ({
    key,
    text: key ? (storage.get(key) ?? "") : "",
  }))
  const text =
    state.key === key ? state.text : key ? (storage.get(key) ?? "") : ""
  const setText = (next: string) => {
    if (key) {
      if (next) storage.set(key, next)
      else {
        try {
          localStorage.removeItem(key)
        } catch {
          /* storage unavailable */
        }
      }
    }
    setState({ key, text: next })
  }
  const submit = async (send: () => void | Promise<void>) => {
    const submitted = text
    try {
      await send()
      // A slow response must not erase text edited while that response was in flight.
      if (key && storage.get(key) === submitted) {
        try {
          localStorage.removeItem(key)
        } catch {
          /* storage unavailable */
        }
      }
      setState((current) =>
        current.key === key && current.text === submitted
          ? { key, text: "" }
          : current
      )
    } catch {
      /* caller presents the error; keep the draft for retry */
    }
  }
  return { text, setText, submit }
}
