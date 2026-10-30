import { Link, useRouterState } from "@tanstack/react-router"
import {
  PanelLeftCloseIcon,
  PanelLeftOpenIcon,
  SettingsIcon,
  UsersRoundIcon,
} from "lucide-react"
import { motion } from "motion/react"

import { Kbd } from "@/components/app/kbd"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { useEndSide } from "@/hooks/use-end-side"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { dur, ease, spring } from "@/lib/motion"
import { cn } from "@/lib/utils"

import { SECTIONS, type SectionKey } from "@/lib/nav"
import { NAV_BADGES, type WorkspaceSummary } from "@/lib/static-workspaces"
import { WorkspaceChip } from "./WorkspaceChip"

const SIDEBAR_WIDTH = 232
const SIDEBAR_WIDTH_COLLAPSED = 56

type ItemProps = {
  label: string
  icon: React.ComponentType<{ className?: string }>
  collapsed: boolean
  active: boolean
  badge?: number
  testId: string
  children: (content: React.ReactNode, className: string) => React.ReactElement
}

function NavItem({
  label,
  icon: Icon,
  collapsed,
  active,
  badge,
  testId,
  children,
}: ItemProps) {
  const reduced = useReducedMotion()
  const endSide = useEndSide()
  const content = (
    <>
      {active ? (
        <motion.span
          layoutId="nav-active"
          transition={reduced ? { duration: 0 } : spring.snappy}
          aria-hidden
          className="absolute inset-y-1 start-0 w-0.5 bg-primary"
        />
      ) : null}
      <Icon className="size-4 shrink-0" />
      {collapsed ? null : (
        <>
          <motion.span
            initial={reduced ? false : { opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: dur.fast, delay: dur.fast, ease: ease.out }}
            className="min-w-0 flex-1 truncate"
          >
            {label}
          </motion.span>
          {badge ? (
            <Badge
              variant="secondary"
              className="tabular-nums"
              data-testid={`${testId}-badge`}
            >
              {badge}
            </Badge>
          ) : null}
        </>
      )}
      {collapsed && badge ? (
        <span
          aria-hidden
          className="absolute end-1.5 top-1.5 size-1.5 bg-primary"
        />
      ) : null}
    </>
  )
  const className = cn(
    "relative flex h-8 items-center gap-2.5 px-2.5 text-[13px] text-sidebar-foreground transition-colors outline-none",
    "hover:bg-sidebar-accent focus-visible:ring-1 focus-visible:ring-sidebar-ring",
    active && "bg-sidebar-accent",
    collapsed && "justify-center px-0"
  )
  const element = children(content, className)
  if (!collapsed) return element
  return (
    <Tooltip>
      <TooltipTrigger asChild>{element}</TooltipTrigger>
      <TooltipContent side={endSide}>
        {label}
        {badge ? ` (${badge})` : ""}
      </TooltipContent>
    </Tooltip>
  )
}

type SidebarProps = {
  workspace: WorkspaceSummary
  collapsed: boolean
  onToggle: () => void
}

export function Sidebar({ workspace, collapsed, onToggle }: SidebarProps) {
  const reduced = useReducedMotion()
  const endSide = useEndSide()
  const pathname = useRouterState({ select: (s) => s.location.pathname })
  const sectionActive = (key: SectionKey) =>
    pathname.startsWith(`/w/${workspace.id}/${key}`)

  return (
    <motion.aside
      data-testid="sidebar"
      data-collapsed={collapsed}
      aria-label="Sidebar"
      initial={false}
      animate={{ width: collapsed ? SIDEBAR_WIDTH_COLLAPSED : SIDEBAR_WIDTH }}
      transition={reduced ? { duration: 0 } : spring.gentle}
      className="flex shrink-0 flex-col overflow-hidden border-e bg-sidebar text-sidebar-foreground"
    >
      <div className="p-2">
        <WorkspaceChip workspace={workspace} collapsed={collapsed} />
      </div>
      <nav
        aria-label="Workspace sections"
        className="flex flex-1 flex-col gap-0.5 px-2 py-1"
      >
        {SECTIONS.map(({ key, label, icon }) => (
          <NavItem
            key={key}
            label={label}
            icon={icon}
            collapsed={collapsed}
            active={sectionActive(key)}
            badge={
              key === "learn" || key === "review" || key === "ingest"
                ? NAV_BADGES[key]
                : undefined
            }
            testId={`nav-${key}`}
          >
            {(content, className) => (
              <Link
                to={`/w/$ws/${key}`}
                params={{ ws: workspace.id }}
                data-testid={`nav-${key}`}
                aria-label={collapsed ? label : undefined}
                aria-current={sectionActive(key) ? "page" : undefined}
                className={className}
              >
                {content}
              </Link>
            )}
          </NavItem>
        ))}
      </nav>
      <nav
        aria-label="Global"
        className="flex flex-col gap-0.5 border-t px-2 py-2"
      >
        {(
          [
            {
              to: "/accounts",
              label: "Accounts",
              icon: UsersRoundIcon,
              testId: "nav-accounts",
            },
            {
              to: "/settings",
              label: "Settings",
              icon: SettingsIcon,
              testId: "nav-settings",
            },
          ] as const
        ).map(({ to, label, icon, testId }) => (
          <NavItem
            key={to}
            label={label}
            icon={icon}
            collapsed={collapsed}
            active={pathname.startsWith(to)}
            testId={testId}
          >
            {(content, className) => (
              <Link
                to={to}
                data-testid={testId}
                aria-label={collapsed ? label : undefined}
                aria-current={pathname.startsWith(to) ? "page" : undefined}
                className={className}
              >
                {content}
              </Link>
            )}
          </NavItem>
        ))}
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              onClick={onToggle}
              data-testid="sidebar-toggle"
              aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
              aria-expanded={!collapsed}
              className={cn(
                "mt-1 h-8 justify-start gap-2.5 px-2.5 text-[13px] text-muted-foreground",
                collapsed && "justify-center px-0"
              )}
            >
              {collapsed ? (
                <PanelLeftOpenIcon className="rtl:-scale-x-100" />
              ) : (
                <PanelLeftCloseIcon className="rtl:-scale-x-100" />
              )}
              {collapsed ? null : (
                <>
                  <span className="flex-1 text-start">Collapse</span>
                  <Kbd combo="[" />
                </>
              )}
            </Button>
          </TooltipTrigger>
          <TooltipContent side={endSide}>
            {collapsed ? "Expand sidebar" : "Collapse sidebar"} [
          </TooltipContent>
        </Tooltip>
      </nav>
    </motion.aside>
  )
}
