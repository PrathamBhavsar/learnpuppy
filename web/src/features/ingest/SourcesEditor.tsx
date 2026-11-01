import { FolderSearchIcon, PlusIcon, SaveIcon, XIcon } from "lucide-react"
import { useState } from "react"

import type {
  IngestCandidate,
  IngestSources,
  IngestSourcesBody,
} from "@/api/pending-types"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { InfoTip } from "@/components/app/info/info"
import { Label } from "@/components/ui/label"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { cn } from "@/lib/utils"

import { ingestInfo } from "./info"
import {
  SOURCE_LABEL,
  normalizePath,
  sinceOptionsFor,
  validatePath,
} from "./model"

const TOOLS = ["claude", "codex", "agy", "git"] as const

type SourcesEditorProps = {
  draft: IngestSourcesBody
  onChange: (next: IngestSourcesBody) => void
  /** Server view: detection candidates and the overlap warnings from the last save/load. */
  server: IngestSources
  dirty: boolean
  saving: boolean
  onSave: () => void
  onReset: () => void
  /** Per-path messages from a rejected save, keyed by allow-list index. */
  pathErrors: Record<number, string>
}

export function SourcesEditor({
  draft,
  onChange,
  server,
  dirty,
  saving,
  onSave,
  onReset,
  pathErrors,
}: SourcesEditorProps) {
  const [manual, setManual] = useState("")
  const [manualError, setManualError] = useState<string | null>(null)
  const [detecting, setDetecting] = useState(false)

  const addManual = () => {
    const error = validatePath(manual, draft.allow)
    if (error) {
      setManualError(error)
      return
    }
    onChange({ ...draft, allow: [...draft.allow, normalizePath(manual)] })
    setManual("")
    setManualError(null)
  }

  const overlapsFor = (path: string) =>
    server.overlaps.filter((o) => o.path === path)

  return (
    <section
      aria-labelledby="sources-heading"
      data-testid="ingest-sources"
      className="flex flex-col gap-5 border p-4"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex flex-col gap-1">
          <h2
            id="sources-heading"
            className="flex items-center gap-1.5 font-heading text-[16px] font-medium"
          >
            Sources
            <InfoTip info={ingestInfo.tools} />
          </h2>
          <p className="text-[13px] text-muted-foreground">
            Which tools to read, and which project folders belong to this
            workspace.
          </p>
        </div>
        <div className="flex items-center gap-2">
          {dirty ? (
            <Button
              variant="outline"
              size="sm"
              onClick={onReset}
              disabled={saving}
              data-testid="sources-reset"
            >
              Reset
            </Button>
          ) : null}
          <Button
            size="sm"
            disabled={!dirty || saving}
            onClick={onSave}
            data-testid="sources-save"
          >
            {saving ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <SaveIcon data-icon="inline-start" />
            )}
            Save sources
          </Button>
          <InfoTip info={ingestInfo.save} />
        </div>
      </div>

      <fieldset className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <legend className="sr-only">Tools to read</legend>
        {TOOLS.map((tool) => (
          <label
            key={tool}
            className="flex items-center justify-between gap-2 border px-3 py-2 text-[13px]"
          >
            {SOURCE_LABEL[tool]}
            <Switch
              size="sm"
              checked={draft[tool]}
              data-testid={`source-toggle-${tool}`}
              aria-label={`Read ${SOURCE_LABEL[tool]}`}
              onCheckedChange={(on) => onChange({ ...draft, [tool]: on })}
            />
          </label>
        ))}
      </fieldset>

      <div className="flex flex-wrap items-center gap-3">
        <span className="flex items-center gap-1.5">
          <span id="since-label" className="text-[13px] text-muted-foreground">
            Look back
          </span>
          <InfoTip info={ingestInfo.since} />
        </span>
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          spacing={0}
          aria-labelledby="since-label"
          value={String(draft.sinceDays)}
          onValueChange={(value) => {
            if (value) onChange({ ...draft, sinceDays: Number(value) })
          }}
          data-testid="since-select"
        >
          {sinceOptionsFor(draft.sinceDays).map((o) => (
            <ToggleGroupItem
              key={o.days}
              value={String(o.days)}
              data-testid={`since-${o.days}`}
            >
              {o.label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </div>

      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between gap-2">
          <span className="flex items-center gap-1.5">
            <Label htmlFor="manual-path">Project folders</Label>
            <InfoTip info={ingestInfo.folders} />
          </span>
          <span className="flex items-center gap-1.5">
            <Button
              variant="outline"
              size="sm"
              data-testid="detect-projects"
              onClick={() => setDetecting(true)}
            >
              <FolderSearchIcon data-icon="inline-start" />
              Detect projects
            </Button>
            <InfoTip info={ingestInfo.detect} />
          </span>
        </div>

        {server.overlaps.length > 0 ? (
          <Alert data-testid="overlap-warning" className="border-warning/60">
            <AlertTitle>Folders shared with another workspace</AlertTitle>
            <AlertDescription>
              <ul role="list" className="flex flex-col gap-0.5">
                {server.overlaps.map((o) => (
                  <li key={`${o.path}-${o.otherPath}`}>
                    <code>{o.path}</code> overlaps <code>{o.otherPath}</code> in
                    another workspace. Sessions there will be read twice.
                  </li>
                ))}
              </ul>
            </AlertDescription>
          </Alert>
        ) : null}

        {draft.allow.length === 0 ? (
          <p
            data-testid="allow-empty"
            className="border border-dashed px-3 py-4 text-[13px] text-muted-foreground"
          >
            No folders yet. Ingest only reads sessions from folders on this
            list, so add at least one before running. Use Detect projects to
            pick from what your tools have already seen.
          </p>
        ) : (
          <ul
            role="list"
            data-testid="allow-list"
            className="flex flex-col gap-1.5"
          >
            {draft.allow.map((path, index) => {
              const error = pathErrors[index]
              const overlap = overlapsFor(path).length > 0
              return (
                <li
                  key={path}
                  data-testid={`allow-item-${index}`}
                  className={cn(
                    "flex flex-col gap-0.5 border px-2.5 py-1.5",
                    error && "border-destructive",
                    !error && overlap && "border-warning/60"
                  )}
                >
                  <div className="flex items-center gap-2">
                    <code
                      className="min-w-0 flex-1 truncate text-[12px]"
                      title={path}
                    >
                      {path}
                    </code>
                    <button
                      type="button"
                      aria-label={`Remove ${path}`}
                      data-testid={`allow-remove-${index}`}
                      className="grid size-6 place-items-center text-muted-foreground hover:text-foreground"
                      onClick={() =>
                        onChange({
                          ...draft,
                          allow: draft.allow.filter((p) => p !== path),
                        })
                      }
                    >
                      <XIcon className="size-3.5" />
                    </button>
                  </div>
                  {error ? (
                    <p
                      role="alert"
                      data-testid={`allow-error-${index}`}
                      className="text-[11px] text-destructive"
                    >
                      {error}
                    </p>
                  ) : null}
                </li>
              )
            })}
          </ul>
        )}

        <div className="flex items-start gap-2">
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <Input
              id="manual-path"
              data-testid="manual-path"
              placeholder="/Users/you/code/project"
              spellCheck={false}
              autoComplete="off"
              value={manual}
              aria-invalid={manualError ? true : undefined}
              aria-describedby={manualError ? "manual-path-error" : undefined}
              onChange={(e) => {
                setManual(e.target.value)
                setManualError(null)
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault()
                  addManual()
                }
              }}
            />
            {manualError ? (
              <p
                id="manual-path-error"
                role="alert"
                data-testid="manual-path-error"
                className="text-[12px] text-destructive"
              >
                {manualError}
              </p>
            ) : null}
          </div>
          <Button
            variant="outline"
            onClick={addManual}
            data-testid="manual-add"
          >
            <PlusIcon data-icon="inline-start" />
            Add
          </Button>
        </div>
      </div>

      <DetectDialog
        open={detecting}
        onOpenChange={setDetecting}
        candidates={server.candidates}
        taken={draft.allow}
        onAdd={(paths) =>
          onChange({ ...draft, allow: [...draft.allow, ...paths] })
        }
      />
    </section>
  )
}

function DetectDialog({
  open,
  onOpenChange,
  candidates,
  taken,
  onAdd,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  candidates: IngestCandidate[]
  taken: string[]
  onAdd: (paths: string[]) => void
}) {
  const [picked, setPicked] = useState<Set<string>>(new Set())
  const available = candidates.filter((c) => !taken.includes(c.path))

  const close = (next: boolean) => {
    if (!next) setPicked(new Set())
    onOpenChange(next)
  }
  const toggle = (path: string, on: boolean) =>
    setPicked((prev) => {
      const next = new Set(prev)
      if (on) next.add(path)
      else next.delete(path)
      return next
    })

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent data-testid="detect-dialog" className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Detected projects</DialogTitle>
          <DialogDescription>
            Folders your tools have recorded sessions in. Pick the ones that
            belong to this workspace.
          </DialogDescription>
        </DialogHeader>
        {available.length === 0 ? (
          <p
            data-testid="detect-empty"
            className="border border-dashed px-3 py-6 text-center text-[13px] text-muted-foreground"
          >
            {candidates.length === 0
              ? "Nothing detected. Use the path field to add a folder by hand."
              : "Every detected project is already on the list."}
          </p>
        ) : (
          <ul
            role="list"
            className="flex max-h-72 flex-col overflow-y-auto border"
          >
            {available.map((c) => (
              <li
                key={c.path}
                className="border-b last:border-b-0"
                data-testid={`candidate-${c.path}`}
              >
                <div className="flex items-center gap-3 px-3 py-2">
                  <Checkbox
                    id={`candidate-box-${c.path}`}
                    checked={picked.has(c.path)}
                    onCheckedChange={(on) => toggle(c.path, on === true)}
                  />
                  <Label
                    htmlFor={`candidate-box-${c.path}`}
                    className="flex min-w-0 flex-1 cursor-pointer items-center"
                  >
                    <span className="flex min-w-0 flex-1 flex-col">
                      <code className="truncate text-[12px]">{c.path}</code>
                      <span className="text-[11px] text-muted-foreground tabular-nums">
                        {SOURCE_LABEL[c.source] ?? c.source} · {c.sessions}{" "}
                        {c.sessions === 1 ? "session" : "sessions"}
                      </span>
                    </span>
                  </Label>
                </div>
              </li>
            ))}
          </ul>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={() => close(false)}>
            Cancel
          </Button>
          <Button
            disabled={picked.size === 0}
            data-testid="detect-add"
            onClick={() => {
              onAdd([...picked])
              close(false)
            }}
          >
            Add {picked.size > 0 ? picked.size : ""} selected
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
