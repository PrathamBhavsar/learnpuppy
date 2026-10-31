import { ClipboardCopyIcon } from "lucide-react"
import { useState } from "react"

import { useAccounts } from "@/api/queries/accounts"
import { useDiagnostics, useSettings } from "@/api/queries/settings"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { toastError, toastSuccess } from "@/lib/toast"

import { buildDiagnostics, redactSecrets } from "./diagnostics"
import { useHealth } from "./health"

/**
 * Builds a report to paste into a bug. The server's `POST /diagnostics` is preferred; when it is not
 * available the report is assembled here from non-sensitive fields. Either way it passes through
 * `redactSecrets` before it is shown or copied.
 */
export function DiagnosticsCard() {
  const health = useHealth()
  const accounts = useAccounts()
  const settings = useSettings()
  const server = useDiagnostics()
  const [text, setText] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const generate = async () => {
    setBusy(true)
    try {
      let report: string
      try {
        report = (await server.mutateAsync()).text
      } catch {
        report = buildDiagnostics({
          version: health.data?.version ?? "unknown",
          engine: health.data?.engine ?? "unknown",
          dataDir: health.data?.dataDir ?? "unknown",
          accounts: accounts.data ?? [],
          settings: settings.data,
          userAgent: navigator.userAgent,
        })
      }
      const safe = redactSecrets(report)
      setText(safe)
      await navigator.clipboard.writeText(safe)
      toastSuccess("Diagnostics copied")
    } catch (error) {
      toastError(error, "Could not copy diagnostics")
    } finally {
      setBusy(false)
    }
  }

  return (
    <div data-testid="settings-diagnostics" className="flex flex-col gap-3">
      <p className="text-[13px] text-muted-foreground">
        Versions, engine mode, account health and role routing. Emails, tokens
        and keys are left out.
      </p>
      <div>
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => void generate()}
          data-testid="copy-diagnostics"
        >
          {busy ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <ClipboardCopyIcon data-icon="inline-start" />
          )}
          Copy diagnostics
        </Button>
      </div>
      {text ? (
        <pre
          data-testid="diagnostics-text"
          className="max-h-60 overflow-auto border bg-muted/40 p-3 text-[12px] whitespace-pre-wrap"
        >
          {text}
        </pre>
      ) : null}
    </div>
  )
}
