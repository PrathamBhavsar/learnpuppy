import { CopyIcon, TriangleAlertIcon } from "lucide-react"
import { useState } from "react"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { cn } from "@/lib/utils"

type ErrorStateProps = {
  title?: string
  message: string
  requestId?: string
  onRetry?: () => void | Promise<unknown>
  className?: string
}

export function ErrorState({
  title = "Something went wrong",
  message,
  requestId,
  onRetry,
  className,
}: ErrorStateProps) {
  const [pending, setPending] = useState(false)

  const retry = async () => {
    if (!onRetry || pending) return
    setPending(true)
    try {
      await onRetry()
    } finally {
      setPending(false)
    }
  }

  return (
    <div
      role="alert"
      data-testid="error-state"
      className={cn(
        "flex flex-col items-center justify-center gap-3 px-6 py-16 text-center",
        className
      )}
    >
      <TriangleAlertIcon
        aria-hidden
        className="lp-shake size-8 text-destructive"
        strokeWidth={1.5}
      />
      <div className="flex max-w-sm flex-col gap-1">
        <h2 className="font-heading text-[16px] font-medium">{title}</h2>
        <p className="text-[13px] text-muted-foreground">{message}</p>
        {requestId ? (
          <p className="text-[11px] text-muted-foreground tabular-nums">
            Request <code>{requestId}</code>
          </p>
        ) : null}
      </div>
      <div className="mt-1 flex items-center gap-2">
        {onRetry ? (
          <Button
            variant="outline"
            onClick={retry}
            disabled={pending}
            data-testid="error-retry"
          >
            {pending ? <Spinner data-icon="inline-start" /> : null}
            Retry
          </Button>
        ) : null}
        {requestId ? (
          <Button
            variant="ghost"
            onClick={() => void navigator.clipboard?.writeText(requestId)}
            data-testid="error-copy-request-id"
          >
            <CopyIcon data-icon="inline-start" />
            Copy request id
          </Button>
        ) : null}
      </div>
    </div>
  )
}
