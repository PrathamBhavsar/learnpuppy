import { useState } from "react"

import { toApiError } from "@/api/errors"
import { useAddClaudeProfile } from "@/api/queries/accounts"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Spinner } from "@/components/ui/spinner"
import { toastError, toastSuccess } from "@/lib/toast"

type AddProfileDialogProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function AddProfileDialog({
  open,
  onOpenChange,
}: AddProfileDialogProps) {
  const add = useAddClaudeProfile()
  const [path, setPath] = useState("")
  const [error, setError] = useState<string | null>(null)

  const close = (next: boolean) => {
    if (!next) {
      setPath("")
      setError(null)
    }
    onOpenChange(next)
  }

  const submit = async (event: React.FormEvent) => {
    event.preventDefault()
    const value = path.trim()
    if (!value.startsWith("/") && !value.startsWith("~")) {
      setError("Enter an absolute path to a CLAUDE_CONFIG_DIR")
      return
    }
    setError(null)
    try {
      await add.mutateAsync(value)
      toastSuccess("Claude profile added")
      close(false)
    } catch (e) {
      const api = toApiError(e)
      const field = api.fields.find((f) => f.path === "path")
      if (field) setError(field.message)
      else toastError(e, "Could not add the profile")
    }
  }

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent data-testid="add-profile-dialog" className="sm:max-w-md">
        <form onSubmit={submit} noValidate className="flex flex-col gap-4">
          <DialogHeader>
            <DialogTitle>Add Claude profile</DialogTitle>
            <DialogDescription>
              A directory holding <code>.claude.json</code>, the value you would
              pass as <code>CLAUDE_CONFIG_DIR</code>. Each profile is its own
              rate-limit bucket.
            </DialogDescription>
          </DialogHeader>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="add-profile-path">Profile directory</Label>
            <Input
              id="add-profile-path"
              data-testid="add-profile-path"
              spellCheck={false}
              autoComplete="off"
              placeholder="/Users/you/.claude-work"
              value={path}
              aria-invalid={error ? true : undefined}
              aria-describedby={error ? "add-profile-error" : undefined}
              onChange={(e) => {
                setPath(e.target.value)
                setError(null)
              }}
            />
            {error ? (
              <p
                id="add-profile-error"
                role="alert"
                data-testid="add-profile-error"
                className="text-[12px] text-destructive"
              >
                {error}
              </p>
            ) : null}
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => close(false)}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              disabled={add.isPending || path.trim() === ""}
              data-testid="add-profile-submit"
            >
              {add.isPending ? <Spinner data-icon="inline-start" /> : null}
              Add profile
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
