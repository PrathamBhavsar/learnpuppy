import type { ErrorComponentProps } from "@tanstack/react-router"

import { toApiError } from "@/api/errors"
import { ErrorState } from "@/components/app/error-state"

/** Router-level error boundary: one broken screen never blanks the shell (docs/web/04 §Errors). */
export function RouteError({ error, reset }: ErrorComponentProps) {
  const api = toApiError(error)
  return (
    <ErrorState
      title="This screen hit a problem"
      message={api.message}
      requestId={api.requestId}
      onRetry={reset}
      className="h-full"
    />
  )
}
