import { LockKeyholeIcon } from "lucide-react"

import { Button } from "@/components/ui/button"

/** Full-screen state when the server rejects the session (docs/web/04 §Errors). */
export function AuthRequired() {
  return (
    <div
      role="alert"
      data-testid="auth-required"
      className="fixed inset-0 z-50 grid place-items-center bg-background p-8"
    >
      <div className="flex max-w-sm flex-col items-center gap-3 text-center">
        <LockKeyholeIcon
          aria-hidden
          className="size-8 text-muted-foreground"
          strokeWidth={1.5}
        />
        <h1 className="font-heading text-[18px] font-medium">
          Open LearnPuppy from your terminal
        </h1>
        <p className="text-[13px] text-muted-foreground">
          This session is no longer signed in. Run <code>learnpuppy serve</code>{" "}
          and open the link it prints; the link carries a one-time launch token.
        </p>
        <Button
          variant="outline"
          onClick={() => location.reload()}
          data-testid="auth-reload"
        >
          I opened the link, reload
        </Button>
      </div>
    </div>
  )
}
