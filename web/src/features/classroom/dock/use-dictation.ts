import { useCallback, useEffect, useRef, useState } from "react"

import { storage } from "@/lib/storage"

type Result = { isFinal: boolean; 0: { transcript: string } }
type RecognitionEvent = { resultIndex: number; results: ArrayLike<Result> }
export type Recognition = {
  continuous: boolean
  interimResults: boolean
  lang: string
  onresult: ((event: RecognitionEvent) => void) | null
  onend: (() => void) | null
  onerror: (() => void) | null
  start: () => void
  stop: () => void
}
type Ctor = new () => Recognition

const ENABLED_KEY = "lp-dictation-on"

function ctor(): Ctor | null {
  const w = globalThis as unknown as {
    SpeechRecognition?: Ctor
    webkitSpeechRecognition?: Ctor
  }
  return w.SpeechRecognition ?? w.webkitSpeechRecognition ?? null
}

/**
 * Browser Web Speech dictation. Off until the learner enables it (the first
 * use shows a privacy notice); appends final transcripts, never submits.
 * No LearnPuppy request is made: audio goes to the browser vendor's service.
 */
export function useDictation(onText: (text: string) => void) {
  const supported = ctor() !== null
  const [enabled, setEnabledState] = useState(
    () => storage.get(ENABLED_KEY) === "1"
  )
  const [listening, setListening] = useState(false)
  const rec = useRef<Recognition | null>(null)
  const sink = useRef(onText)
  useEffect(() => {
    sink.current = onText
  })

  const stop = useCallback(() => {
    const r = rec.current
    rec.current = null
    if (r) {
      r.onresult = null
      r.onend = null
      r.onerror = null
      r.stop()
    }
    setListening(false)
  }, [])

  const start = useCallback(() => {
    const C = ctor()
    if (!C || rec.current) return
    const r = new C()
    r.continuous = true
    r.interimResults = false
    r.lang = document.documentElement.lang || navigator.language || "en-US"
    r.onresult = (event) => {
      let out = ""
      for (let i = event.resultIndex; i < event.results.length; i++) {
        const item = event.results[i]
        if (item?.isFinal) out += item[0].transcript
      }
      if (out.trim()) sink.current(out.trim())
    }
    r.onend = () => {
      rec.current = null
      setListening(false)
    }
    r.onerror = r.onend
    rec.current = r
    setListening(true)
    r.start()
  }, [])

  useEffect(() => stop, [stop])

  const enable = useCallback(() => {
    storage.set(ENABLED_KEY, "1")
    setEnabledState(true)
  }, [])

  return { supported, enabled, enable, listening, start, stop }
}
