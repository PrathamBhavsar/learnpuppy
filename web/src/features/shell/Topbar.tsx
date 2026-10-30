import { Link, useRouterState } from "@tanstack/react-router"
import { SearchIcon } from "lucide-react"
import { Fragment } from "react"

import { JobsPopover } from "@/components/app/jobs-popover"
import { DirectionToggle } from "@/components/app/direction-toggle"
import { Kbd } from "@/components/app/kbd"
import { ThemeToggle } from "@/components/app/theme-toggle"
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb"
import { Button } from "@/components/ui/button"
import { usePalette } from "@/stores/palette"

import { SECTION_LABELS } from "@/lib/nav"
import {
  ACCOUNTS_SUMMARY,
  RUNNING_JOBS,
  type WorkspaceSummary,
} from "@/lib/static-workspaces"

type Crumb = { label: string; to?: string }

export function buildCrumbs(
  pathname: string,
  workspace: WorkspaceSummary
): Crumb[] {
  const segments = pathname.split("/").filter(Boolean)
  const crumbs: Crumb[] = []
  if (segments[0] === "w") {
    crumbs.push({ label: workspace.name, to: `/w/${workspace.id}/graph` })
    const section = segments[2]
    if (section) crumbs.push({ label: SECTION_LABELS[section] ?? section })
    const item = segments[3]
    if (item) crumbs.push({ label: item })
    return crumbs
  }
  const root = segments[0]
  if (root) crumbs.push({ label: SECTION_LABELS[root] ?? root })
  return crumbs
}

function Breadcrumbs({ workspace }: { workspace: WorkspaceSummary }) {
  const pathname = useRouterState({ select: (s) => s.location.pathname })
  const crumbs = buildCrumbs(pathname, workspace)
  return (
    <Breadcrumb className="min-w-0 flex-1" data-testid="breadcrumb">
      <BreadcrumbList>
        {crumbs.map((crumb, i) => {
          const last = i === crumbs.length - 1
          return (
            <Fragment key={`${i}-${crumb.label}`}>
              <BreadcrumbItem>
                {last || !crumb.to ? (
                  <BreadcrumbPage>{crumb.label}</BreadcrumbPage>
                ) : (
                  <BreadcrumbLink asChild>
                    <Link to={crumb.to}>{crumb.label}</Link>
                  </BreadcrumbLink>
                )}
              </BreadcrumbItem>
              {last ? null : (
                <BreadcrumbSeparator className="rtl:-scale-x-100" />
              )}
            </Fragment>
          )
        })}
      </BreadcrumbList>
    </Breadcrumb>
  )
}

function AccountsPill() {
  const { ready, total } = ACCOUNTS_SUMMARY
  const state = ready === 0 ? "none" : ready < total ? "partial" : "all"
  return (
    <Button
      asChild
      variant="outline"
      size="sm"
      className="gap-1.5 tabular-nums"
    >
      <Link to="/accounts" data-testid="accounts-pill" data-state={state}>
        <span
          aria-hidden
          className={
            state === "none"
              ? "size-1.5 bg-destructive"
              : state === "partial"
                ? "size-1.5 bg-warning"
                : "size-1.5 bg-success"
          }
        />
        <span className="sr-only">Accounts </span>
        {ready}/{total} ready
      </Link>
    </Button>
  )
}

export function Topbar({ workspace }: { workspace: WorkspaceSummary }) {
  const setPaletteOpen = usePalette((s) => s.setOpen)
  return (
    <header
      data-testid="topbar"
      className="flex h-12 shrink-0 items-center gap-2 border-b bg-background px-4"
    >
      <Breadcrumbs workspace={workspace} />
      <Button
        variant="outline"
        onClick={() => setPaletteOpen(true)}
        data-testid="search-trigger"
        className="w-56 justify-start gap-2 text-muted-foreground"
      >
        <SearchIcon aria-hidden data-icon="inline-start" />
        <span className="flex-1 text-start">Search…</span>
        <Kbd combo="mod+k" />
      </Button>
      <JobsPopover jobs={RUNNING_JOBS} />
      <AccountsPill />
      <DirectionToggle />
      <ThemeToggle />
    </header>
  )
}
