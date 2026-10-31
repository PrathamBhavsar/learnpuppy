import { Link } from "@tanstack/react-router"
import { CheckIcon, WifiOffIcon } from "lucide-react"
import { AnimatePresence, motion } from "motion/react"
import { useEffect, useState } from "react"

import { Spinner } from "@/components/ui/spinner"
import { useConnection } from "@/stores/connection"
import { useStatus } from "@/stores/status"
import { useAccounts } from "@/api/queries/accounts"

/** The banner only appears once the stream has been down this long (docs/web/06 §Global shell). */
export const BANNER_DELAY_MS = 2000
const RECOVERED_FLASH_MS = 1800

type Phase = "hidden" | "down" | "recovered"

function useBannerPhase(): Phase {
  const status = useConnection((s) => s.status)
  const downSince = useConnection((s) => s.downSince)
  const recoveredAt = useConnection((s) => s.recoveredAt)
  const [now, setNow] = useState(() => Date.now())

  useEffect(() => {
    if (status !== "reconnecting" || downSince === null) return
    const wait = Math.max(0, downSince + BANNER_DELAY_MS - Date.now())
    const timer = setTimeout(() => setNow(Date.now()), wait + 10)
    return () => clearTimeout(timer)
  }, [status, downSince])

  useEffect(() => {
    if (recoveredAt === null) return
    const timer = setTimeout(() => setNow(Date.now()), RECOVERED_FLASH_MS + 10)
    return () => clearTimeout(timer)
  }, [recoveredAt])

  if (
    status === "reconnecting" &&
    downSince !== null &&
    now - downSince >= BANNER_DELAY_MS
  )
    return "down"
  if (
    status === "open" &&
    recoveredAt !== null &&
    now - recoveredAt < RECOVERED_FLASH_MS
  )
    return "recovered"
  return "hidden"
}

/** "Reconnecting… your work is safe" under the topbar while the event stream is down. */
export function ConnectionBanner() {
  const phase = useBannerPhase()
  const attempt = useConnection((s) => s.attempt)
  return (
    <AnimatePresence initial={false}>
      {phase === "hidden" ? null : (
        <motion.div
          key="connection-banner"
          role="status"
          aria-live="polite"
          data-testid="connection-banner"
          data-state={phase}
          initial={{ height: 0, opacity: 0 }}
          animate={{ height: "auto", opacity: 1 }}
          exit={{ height: 0, opacity: 0 }}
          transition={{ duration: 0.18 }}
          className="shrink-0 overflow-hidden border-b"
        >
          <div
            className={
              phase === "down"
                ? "flex items-center gap-2 bg-warning/15 px-4 py-1.5 text-[13px]"
                : "flex items-center gap-2 bg-success/15 px-4 py-1.5 text-[13px]"
            }
          >
            {phase === "down" ? (
              <>
                <Spinner aria-hidden className="size-3.5" />
                <WifiOffIcon aria-hidden className="size-3.5" />
                <span>Reconnecting… your work is safe.</span>
                {attempt > 1 ? (
                  <span className="text-muted-foreground tabular-nums">
                    Attempt {attempt}
                  </span>
                ) : null}
              </>
            ) : (
              <>
                <CheckIcon aria-hidden className="size-3.5" />
                <span>Back online.</span>
              </>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  )
}

/** Inline banner while no engine can answer (every account in cooldown or signed out). */
export function EngineBanner() {
  const engineUnavailable = useStatus((s) => s.engineUnavailable)
  const setEngineUnavailable = useStatus((s) => s.setEngineUnavailable)
  const accounts = useAccounts()
  const anyReady = accounts.data?.some((a) => a.status === "ready") ?? false
  useEffect(() => {
    if (engineUnavailable && anyReady) setEngineUnavailable(false)
  }, [engineUnavailable, anyReady, setEngineUnavailable])
  if (!engineUnavailable || anyReady) return null
  return (
    <div
      role="alert"
      data-testid="engine-banner"
      className="flex shrink-0 items-center gap-2 border-b bg-destructive/10 px-4 py-1.5 text-[13px]"
    >
      <span>No model account is ready, so chats and ingest are paused.</span>
      <Link
        to="/accounts"
        className="underline underline-offset-2"
        data-testid="engine-banner-link"
      >
        Open Accounts
      </Link>
    </div>
  )
}
