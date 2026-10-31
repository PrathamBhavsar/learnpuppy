import { useNavigate } from "@tanstack/react-router"
import { CheckIcon, RefreshCwIcon, UploadIcon } from "lucide-react"
import { useState } from "react"

import { toApiError } from "@/api/errors"
import type { WorkspaceListItem } from "@/api/pending-types"
import { useAccounts, useRescanAccounts } from "@/api/queries/accounts"
import {
  useIngestSources,
  useRunIngest,
  useSaveIngestSources,
} from "@/api/queries/ingest"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Label } from "@/components/ui/label"
import { SkeletonRows } from "@/components/app/skeleton-block"
import { Spinner } from "@/components/ui/spinner"
import { toastError } from "@/lib/toast"
import { cn } from "@/lib/utils"

import { relativeTime } from "./model"
import { WorkspaceForm } from "./WorkspaceDialog"

function Step({
  n,
  title,
  done,
  disabled,
  children,
}: {
  n: number
  title: string
  done?: boolean
  disabled?: boolean
  children: React.ReactNode
}) {
  return (
    <section
      aria-labelledby={`onboarding-step-${n}`}
      data-testid={`onboarding-step-${n}`}
      data-disabled={disabled || undefined}
      className={cn(
        "flex flex-col gap-3 border bg-card p-5 transition-opacity duration-[var(--dur-base)]",
        disabled && "opacity-50"
      )}
    >
      <h2
        id={`onboarding-step-${n}`}
        className="flex items-center gap-2 font-heading text-[14px] font-medium"
      >
        <span
          aria-hidden
          className={cn(
            "grid size-5 place-items-center border text-[11px] tabular-nums",
            done && "border-transparent bg-primary text-primary-foreground"
          )}
        >
          {done ? <CheckIcon className="size-3" /> : n}
        </span>
        {title}
      </h2>
      {children}
    </section>
  )
}

const STATUS_LABEL: Record<string, string> = {
  ready: "Ready",
  cooldown: "Cooling down",
  auth_expired: "Sign-in expired",
  disabled: "Disabled",
}

function AccountsStep() {
  const accounts = useAccounts()
  const rescan = useRescanAccounts()
  const list = accounts.data ?? []
  const ready = list.filter((a) => a.status === "ready").length
  return (
    <Step n={1} title="Detected accounts" done={ready > 0}>
      {accounts.isPending ? (
        <SkeletonRows rows={2} />
      ) : accounts.isError ? (
        <p role="alert" className="text-[13px] text-destructive">
          {toApiError(accounts.error).message}
        </p>
      ) : list.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">
          No CLI account found. Run{" "}
          <code className="bg-muted px-1">claude login</code> in a terminal,
          then rescan.
        </p>
      ) : (
        <ul
          role="list"
          className="flex flex-col gap-1.5"
          data-testid="onboarding-accounts"
        >
          {list.map((account) => (
            <li
              key={account.id}
              className="flex items-center gap-2 text-[13px]"
            >
              <span
                aria-hidden
                className={cn(
                  "size-2 shrink-0 rounded-full",
                  account.status === "ready"
                    ? "bg-success"
                    : account.status === "cooldown"
                      ? "bg-warning"
                      : "bg-destructive"
                )}
              />
              <span className="min-w-0 flex-1 truncate">
                <span className="capitalize">{account.provider}</span>
                {account.email ? ` · ${account.email}` : ""}
              </span>
              <span className="text-[11px] text-muted-foreground">
                {STATUS_LABEL[account.status] ?? account.status}
              </span>
            </li>
          ))}
        </ul>
      )}
      <div>
        <Button
          variant="outline"
          size="sm"
          disabled={rescan.isPending}
          onClick={() =>
            rescan.mutate(undefined, {
              onError: (error) => toastError(error, "Rescan failed"),
            })
          }
          data-testid="onboarding-rescan"
        >
          {rescan.isPending ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <RefreshCwIcon data-icon="inline-start" />
          )}
          Rescan
        </Button>
      </div>
    </Step>
  )
}

function SourcesStep({ workspace }: { workspace: WorkspaceListItem | null }) {
  const navigate = useNavigate()
  const sources = useIngestSources(workspace?.id ?? "")
  const save = useSaveIngestSources(workspace?.id ?? "")
  const run = useRunIngest(workspace?.id ?? "")
  const [picked, setPicked] = useState<Set<string>>(new Set())
  const [busy, setBusy] = useState(false)
  const candidates = sources.data?.candidates ?? []
  const suggested = new Set(
    [...candidates]
      .sort((a, b) => b.sessions - a.sessions)
      .slice(0, 3)
      .map((c) => c.path)
  )

  const open = (section: "graph" | "ingest") => {
    if (workspace)
      void navigate({
        to: section === "graph" ? "/w/$ws/graph" : "/w/$ws/ingest",
        params: { ws: workspace.id },
      })
  }

  const start = async () => {
    if (!sources.data || picked.size === 0) return
    setBusy(true)
    try {
      const { claude, codex, agy, git, sinceDays } = sources.data
      await save.mutateAsync({
        claude,
        codex,
        agy,
        git,
        sinceDays,
        allow: [...picked],
      })
      await run.mutateAsync(undefined)
      open("ingest")
    } catch (error) {
      toastError(error, "Could not start ingest")
    } finally {
      setBusy(false)
    }
  }

  return (
    <Step
      n={3}
      title="Choose sources"
      disabled={!workspace}
      done={picked.size > 0}
    >
      {!workspace ? (
        <p className="text-[13px] text-muted-foreground">
          Create the workspace first; project folders are detected from your
          recent sessions.
        </p>
      ) : sources.isPending ? (
        <SkeletonRows rows={3} />
      ) : candidates.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">
          No project folders found in recent sessions. You can add them later
          under Ingest.
        </p>
      ) : (
        <ul
          role="list"
          className="flex flex-col gap-1.5"
          data-testid="onboarding-candidates"
        >
          {candidates.map((candidate, i) => {
            const id = `candidate-${i}`
            return (
              <li key={candidate.path} className="flex items-center gap-2">
                <Checkbox
                  id={id}
                  checked={picked.has(candidate.path)}
                  onCheckedChange={(checked) =>
                    setPicked((prev) => {
                      const next = new Set(prev)
                      if (checked === true) next.add(candidate.path)
                      else next.delete(candidate.path)
                      return next
                    })
                  }
                  data-testid={`onboarding-candidate-${i}`}
                />
                <Label
                  htmlFor={id}
                  className="min-w-0 flex-1 flex-wrap gap-x-2 text-[13px] font-normal"
                >
                  <span className="truncate">{candidate.path}</span>
                  <span className="text-[11px] text-muted-foreground tabular-nums">
                    {candidate.sessions} sessions ·{" "}
                    {relativeTime(candidate.lastSeen)}
                  </span>
                  {suggested.has(candidate.path) ? (
                    <Badge variant="secondary">Suggested</Badge>
                  ) : null}
                </Label>
              </li>
            )
          })}
        </ul>
      )}
      {workspace ? (
        <div className="flex gap-2">
          <Button
            disabled={picked.size === 0 || busy}
            onClick={() => void start()}
            data-testid="onboarding-start"
          >
            {busy ? <Spinner data-icon="inline-start" /> : null}
            Save sources &amp; start ingest
          </Button>
          <Button
            variant="ghost"
            onClick={() => open("graph")}
            data-testid="onboarding-skip"
          >
            Skip for now
          </Button>
        </div>
      ) : null}
    </Step>
  )
}

/** First run: accounts, first workspace, sources: three sections on one page, skippable at the last. */
export function Onboarding({ onImport }: { onImport: () => void }) {
  const [created, setCreated] = useState<WorkspaceListItem | null>(null)
  return (
    <div className="flex flex-col gap-4" data-testid="onboarding">
      <p className="text-[13px] text-muted-foreground">
        Three quick steps and you are learning. Nothing here is permanent; you
        can change all of it later.
      </p>
      <AccountsStep />
      <Step n={2} title="Name your first workspace" done={created !== null}>
        {created ? (
          <p className="text-[13px]" data-testid="onboarding-created">
            Created <strong>{created.name}</strong>.
          </p>
        ) : (
          <WorkspaceForm
            existing={[]}
            cancellable={false}
            focusName={false}
            submitLabel="Create workspace"
            onDone={(workspace) => {
              if (workspace) setCreated(workspace)
            }}
          />
        )}
      </Step>
      <SourcesStep workspace={created} />
      <div>
        <Button
          variant="ghost"
          size="sm"
          onClick={onImport}
          data-testid="onboarding-import"
        >
          <UploadIcon data-icon="inline-start" />
          Import an exported workspace instead
        </Button>
      </div>
    </div>
  )
}
