import { Link } from "@tanstack/react-router"
import {
  ClockIcon,
  CopyIcon,
  RotateCcwIcon,
  TriangleAlertIcon,
} from "lucide-react"

import type { ClassroomView } from "@/api/pending-types"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
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
  ].join("\n")
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
      className="mx-auto flex w-full max-w-3xl flex-col gap-2 border-t border-destructive/40 bg-destructive/10 px-4 py-3 text-[13px]"
    >
      <div className="flex items-start gap-2">
        <TriangleAlertIcon
          aria-hidden
          className="mt-0.5 size-4 shrink-0 text-destructive"
        />
        <div className="min-w-0 flex-1">
          <p className="font-medium">
            {interrupted
              ? "The lesson was interrupted"
              : "The tutor could not answer"}
          </p>
          <p className="text-muted-foreground">
            {error?.message ?? "Something went wrong while running this turn."}
          </p>
          {error?.code === "engine_unavailable" ? (
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
          {interrupted ? COPY.resume : COPY.retry}
        </Button>
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
