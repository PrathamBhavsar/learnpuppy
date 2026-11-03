import { CheckIcon, CopyIcon } from "lucide-react"
import { useState } from "react"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { toastError } from "@/lib/toast"

import { CONNECT_COMMAND, PROVIDER_BLURB, providerTitle } from "@/lib/accounts-model"

type ConnectDialogProps = {
  provider: string | null
  /** The server's exact command for this account, when it has one. */
  command?: string | null
  onOpenChange: (open: boolean) => void
  onRescan?: () => void
}

/**
 * "How to connect": the exact command to run in your own terminal. LearnPuppy never runs it for you,
 * so a login stays under your control; press Rescan once it finished.
 */
export function ConnectDialog({
  provider,
  command,
  onOpenChange,
  onRescan,
}: ConnectDialogProps) {
  const [copied, setCopied] = useState(false)
  const cmd = command || (provider ? CONNECT_COMMAND[provider] : "") || ""

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(cmd)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    } catch (error) {
      toastError(error, "Could not copy the command")
    }
  }

  return (
    <Dialog open={provider !== null} onOpenChange={onOpenChange}>
      <DialogContent data-testid="connect-dialog">
        <DialogHeader>
          <DialogTitle>
            Connect {provider ? providerTitle(provider) : ""}
          </DialogTitle>
          <DialogDescription>
            {provider ? PROVIDER_BLURB[provider] : ""}
          </DialogDescription>
        </DialogHeader>
        <ol className="flex list-decimal flex-col gap-2 ps-5 text-[13px]">
          <li>Open a terminal on this computer.</li>
          <li>
            Run this command and finish the sign-in it opens:
            <div className="mt-1.5 flex items-center gap-2 border bg-muted/40 px-2 py-1.5">
              <code
                data-testid="connect-command"
                className="min-w-0 flex-1 truncate text-[12px]"
                title={cmd}
              >
                {cmd}
              </code>
              <button
                type="button"
                onClick={() => void copy()}
                aria-label="Copy the command"
                data-testid="connect-copy"
                className="grid size-6 place-items-center text-muted-foreground hover:text-foreground"
              >
                {copied ? (
                  <CheckIcon className="size-3.5" />
                ) : (
                  <CopyIcon className="size-3.5" />
                )}
              </button>
            </div>
          </li>
          <li>Come back here and press Rescan.</li>
        </ol>
        <p className="text-[12px] text-muted-foreground">
          LearnPuppy never runs this for you and never sees your password.
        </p>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
          {onRescan ? (
            <Button data-testid="connect-rescan" onClick={onRescan}>
              Rescan now
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
