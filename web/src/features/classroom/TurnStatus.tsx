import { Link } from "@tanstack/react-router"
import {
  ClockIcon,
  CopyIcon,
  RotateCcwIcon,
  TriangleAlertIcon,
  WifiOffIcon,
} from "lucide-react"
import { useEffect, useState } from "react"

import type { ClassroomView } from "@/api/pending-types"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { providerTitle } from "@/lib/accounts-model"
import { formatResets } from "@/lib/ai-time"
import { toastInfo } from "@/lib/toast"

import { COPY } from "./copy"
import { useNow } from "./use-now"

type Turn = ClassroomView["turn"]

type RateLimitBannerProps = {
  turn: Turn
  onCancel: () => void
  cancelling: boolean
}

/** The turn is queued behind a model-account cooldown: live countdown, with cancel. */
export function RateLimitBanner({
  turn,
  onCancel,
  cancelling,
}: RateLimitBannerProps) {
  const now = useNow(true)
  const retryAt = turn.retryAt ?? null
  const remaining =
    retryAt === null ? null : Math.max(0, (retryAt - now) / 1000)
  return (
    <div
      role="status"
      data-testid="rate-limit-banner"
      className="mx-auto flex w-full max-w-3xl items-center gap-3 border-t border-warning/40 bg-warning/10 px-4 py-2 text-[13px]"
    >
      <ClockIcon aria-hidden className="size-4 shrink-0 text-warning" />
      <p className="min-w-0 flex-1">
        {COPY.waiting}
        {remaining !== null ? (
          <>
            {" "}
            (≈{" "}
            <span data-testid="rate-limit-countdown" className="tabular-nums">
              {Math.ceil(remaining)} s
            </span>
            )
          </>
        ) : null}
        .
      </p>
      <Button
        variant="outline"
        size="sm"
        onClick={onCancel}
        disabled={cancelling}
        data-testid="rate-limit-cancel"
      >
        {cancelling ? <Spinner data-icon="inline-start" /> : null}
        Cancel
      </Button>
    </div>
  )
}

type TurnErrorCardProps = {
  chatId: string
  turn: Turn
  viewVersion: number
  onRetry: () => void
  retrying: boolean
}

function diagnostics(chatId: string, turn: Turn, viewVersion: number): string {
  const e = turn.error
  return [
    `chat: ${chatId}`,
    `turn: ${turn.turnId ?? "unknown"}`,
    `viewVersion: ${viewVersion}`,
    `code: ${e?.code ?? "unknown"}`,
    `message: ${e?.message ?? "unknown"}`,
    `retryable: ${e?.retryable ?? false}`,
    `interrupted: ${e?.interrupted ?? false}`,
    ...(e?.provider ? [`provider: ${e.provider}`] : []),
    ...(e?.model ? [`model: ${e.model}`] : []),
  ].join("\n")
}

/** Seconds between automatic retries while offline: 5, 10, 20, 40, then 60. */
function offlineDelaySecs(attempt: number, hintSecs?: number | null) {
  const base = Math.max(5, hintSecs ?? 5)
  return Math.min(60, base * 2 ** Math.min(attempt, 4))
}

const offlineAttempts = new Map<string, number>()

type Friendly = {
  title: string
  body: React.ReactNode
  /** Hide the Retry button (the user has to fix something first). */
  noRetry?: boolean
}

function describe(
  turn: Turn,
  now: number,
  offlineLeft: number | null
): Friendly | null {
  const e = turn.error
  if (!e) return null
  const who = e.provider
    ? `${providerTitle(e.provider)}${e.model ? ` (${e.model})` : ""}`
    : "Your AI"
  switch (e.code) {
    case "OFFLINE":
      return {
        title: "You're offline",
        body: (
          <>
            No AI can be reached right now, so nothing is lost. Retrying in{" "}
            <span data-testid="offline-countdown" className="tabular-nums">
              {offlineLeft ?? 0}s
            </span>
            ; it also resumes by itself when you are back online.
          </>
        ),
      }
    case "NOT_CONNECTED":
      return {
        title: "No AI is connected",
        noRetry: true,
        body: (
          <>
            {e.message}{" "}
            {e.action ? (
              <>
                Run <code data-testid="howto-command">{e.action}</code> in a
                terminal, then press Rescan on{" "}
              </>
            ) : (
              <>Connect one on </>
            )}
            <Link
              to="/accounts"
              data-testid="turn-open-accounts"
              className="text-primary underline underline-offset-2"
            >
              Accounts
            </Link>
            .
          </>
        ),
      }
    case "NO_MODEL":
      return {
        title: "That model is not available",
        body: (
          <>
            {who} has no model that can run this. Choose another model in{" "}
            <Link
              to="/accounts"
              data-testid="turn-open-settings"
              className="text-primary underline underline-offset-2"
            >
              Role slots
            </Link>
            , then retry.
          </>
        ),
      }
    case "RATE_LIMITED":
    case "rate_limited": {
      const at = e.resetsAt ?? (e.retryAfterMs ? now + e.retryAfterMs : null)
      return {
        title: "Your AI is limited",
        body: (
          <>
            {who} is back first
            {at ? (
              <>
                :{" "}
                <span data-testid="limit-resets">{formatResets(at, now)}</span>
              </>
            ) : null}
            . Retry once it returns, or pick another AI in Role slots.
          </>
        ),
      }
    }
    case "TIMED_OUT":
      return {
        title: "The AI took too long",
        body: "It did not answer in time. Try again, or pick a faster model in Role slots.",
      }
    case "NO_RESPONSE":
      return {
        title: "The tutor had nothing to say",
        body: "The AI returned an empty answer. Try again.",
      }
    default:
      return null
  }
}

/** The last turn failed. Retry re-posts the same `clientId`; a restart-interrupted turn offers Resume. */
export function TurnErrorCard({
  chatId,
  turn,
  viewVersion,
  onRetry,
  retrying,
}: TurnErrorCardProps) {
  const error = turn.error
  const interrupted = error?.interrupted ?? false
  const offline = error?.code === "OFFLINE"
  const now = useNow(true)
  const [shownAt, setShownAt] = useState(() => Date.now())
  const attempt = offlineAttempts.get(chatId) ?? 0
  const delay = offlineDelaySecs(attempt, error?.retryInSecs)
  const offlineLeft = offline
    ? Math.max(0, Math.ceil(delay - (now - shownAt) / 1000))
    : null

  // Offline: retry by itself with backoff, so a flaky network recovers without a click.
  useEffect(() => {
    if (!offline) {
      offlineAttempts.delete(chatId)
      return
    }
    // Restart the countdown for each new failed attempt (a timestamp, not derivable in render).
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setShownAt(Date.now())
    const timer = window.setTimeout(() => {
      offlineAttempts.set(chatId, attempt + 1)
      onRetry()
    }, delay * 1000)
    return () => window.clearTimeout(timer)
    // A new error object means a new failed attempt.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [offline, turn.turnId, error?.retryInSecs])

  const friendly = interrupted ? null : describe(turn, now, offlineLeft)
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(
        diagnostics(chatId, turn, viewVersion)
      )
      toastInfo("Diagnostics copied")
    } catch {
      toastInfo("Could not copy", "Clipboard access was denied.")
    }
  }
  return (
    <div
      role="alert"
      data-testid="turn-error"
      data-code={error?.code}
      className={
        offline || friendly
          ? "mx-auto flex w-full max-w-3xl flex-col gap-2 border-t border-warning/40 bg-warning/10 px-4 py-3 text-[13px]"
          : "mx-auto flex w-full max-w-3xl flex-col gap-2 border-t border-destructive/40 bg-destructive/10 px-4 py-3 text-[13px]"
      }
    >
      <div className="flex items-start gap-2">
        {offline ? (
          <WifiOffIcon
            aria-hidden
            className="mt-0.5 size-4 shrink-0 text-warning"
          />
        ) : (
          <TriangleAlertIcon
            aria-hidden
            className={
              friendly
                ? "mt-0.5 size-4 shrink-0 text-warning"
                : "mt-0.5 size-4 shrink-0 text-destructive"
            }
          />
        )}
        <div className="min-w-0 flex-1">
          <p className="font-medium" data-testid="turn-error-title">
            {interrupted
              ? "The lesson was interrupted"
              : (friendly?.title ?? "The tutor could not answer")}
          </p>
          <p className="text-muted-foreground" data-testid="turn-error-body">
            {friendly?.body ??
              error?.message ??
              "Something went wrong while running this turn."}
          </p>
          {!friendly && error?.code === "engine_unavailable" ? (
            <Link
              to="/accounts"
              className="text-primary underline underline-offset-2"
            >
              Open Accounts
            </Link>
          ) : null}
        </div>
      </div>
      <div className="flex items-center gap-2 ps-6">
        {friendly?.noRetry ? null : (
          <Button
            size="sm"
            onClick={onRetry}
            disabled={retrying}
            data-testid={interrupted ? "turn-resume" : "turn-retry"}
          >
            {retrying ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <RotateCcwIcon data-icon="inline-start" />
            )}
            {interrupted ? COPY.resume : offline ? "Retry now" : COPY.retry}
          </Button>
        )}
        <Button
          variant="ghost"
          size="sm"
          onClick={() => void copy()}
          data-testid="turn-copy-diagnostics"
        >
          <CopyIcon data-icon="inline-start" />
          {COPY.copyDiagnostics}
        </Button>
      </div>
    </div>
  )
}
