import { Link, useRouterState } from "@tanstack/react-router"
import { SearchIcon } from "lucide-react"
import { Fragment } from "react"

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
import { HelpMenu } from "@/components/app/help-menu"
import { SHELL_INFO } from "@/components/app/info/shell-content"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { usePalette } from "@/stores/palette"

import type { WorkspaceListItem } from "@/api/pending-types"

import { SECTION_LABELS } from "@/lib/nav"
import { AccountsPill } from "./AccountsPill"
import { WorkspaceJobs } from "./WorkspaceJobs"

type Crumb = { label: string; to?: string }

export function buildCrumbs(
  pathname: string,
  workspace: Pick<WorkspaceListItem, "id" | "name"> | undefined
): Crumb[] {
  const segments = pathname.split("/").filter(Boolean)
  const crumbs: Crumb[] = []
  if (segments[0] === "w") {
    crumbs.push({
      label: workspace?.name ?? segments[1] ?? "Workspace",
      to: `/w/${segments[1]}/graph`,
    })
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

function Breadcrumbs({
  workspace,
}: {
  workspace: Pick<WorkspaceListItem, "id" | "name"> | undefined
}) {
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

export function Topbar({
  workspace,
}: {
  workspace: Pick<WorkspaceListItem, "id" | "name"> | undefined
}) {
  const setPaletteOpen = usePalette((s) => s.setOpen)
  return (
    <header
      data-testid="topbar"
      className="flex h-12 shrink-0 items-center gap-2 border-b bg-background px-4"
    >
      <Breadcrumbs workspace={workspace} />
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="outline"
            onClick={() => setPaletteOpen(true)}
            data-testid="search-trigger"
            data-tour="search"
            className="w-56 justify-start gap-2 text-muted-foreground"
          >
            <SearchIcon aria-hidden data-icon="inline-start" />
            <span className="flex-1 text-start">Search…</span>
            <Kbd combo="mod+k" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{SHELL_INFO.search.short}</TooltipContent>
      </Tooltip>
      <WorkspaceJobs workspaceId={workspace?.id ?? null} />
      <AccountsPill />
      <div className="flex items-center gap-2" data-tour="appearance">
        <DirectionToggle />
        <ThemeToggle />
      </div>
      <HelpMenu />
    </header>
  )
}
