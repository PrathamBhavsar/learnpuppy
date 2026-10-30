import { Outlet, useParams, useRouterState } from "@tanstack/react-router"
import { motion } from "motion/react"
import { useEffect, useRef, useState } from "react"

import { useHotkey } from "@/hooks/use-hotkey"
import { fadeRise } from "@/lib/motion"
import { useUi } from "@/stores/ui"

import { Sidebar } from "./Sidebar"
import { DEFAULT_WORKSPACE, findWorkspace } from "@/lib/static-workspaces"
import { Topbar } from "./Topbar"

/** Moves focus to the page h1 and announces the new title after each navigation. */
function useRouteAnnouncer() {
  const pathname = useRouterState({ select: (s) => s.location.pathname })
  const [announcement, setAnnouncement] = useState("")
  const first = useRef(true)
  useEffect(() => {
    if (first.current) {
      first.current = false
      return
    }
    const frame = requestAnimationFrame(() => {
      document
        .querySelector<HTMLElement>("[data-page-title]")
        ?.focus({ preventScroll: true })
      setAnnouncement(document.title)
    })
    return () => cancelAnimationFrame(frame)
  }, [pathname])
  return announcement
}

export function AppShell() {
  const collapsed = useUi((s) => s.sidebarCollapsed)
  const toggleSidebar = useUi((s) => s.toggleSidebar)
  const lastWorkspace = useUi((s) => s.lastWorkspace)
  const setLastWorkspace = useUi((s) => s.setLastWorkspace)
  const { ws } = useParams({ strict: false })
  const section = useRouterState({
    select: (s) => s.location.pathname.split("/")[ws ? 3 : 1] ?? "",
  })
  const announcement = useRouteAnnouncer()

  const workspace =
    findWorkspace(ws) ?? findWorkspace(lastWorkspace) ?? DEFAULT_WORKSPACE
  useEffect(() => {
    if (ws && findWorkspace(ws)) setLastWorkspace(ws)
  }, [ws, setLastWorkspace])

  useHotkey("[", toggleSidebar)

  return (
    <div
      className="flex h-dvh w-dvw overflow-hidden bg-background text-foreground"
      data-testid="app-shell"
    >
      <a
        href="#main"
        className="sr-only z-50 bg-primary px-3 py-2 text-primary-foreground focus:not-sr-only focus:absolute focus:start-2 focus:top-2"
      >
        Skip to content
      </a>
      <Sidebar
        workspace={workspace}
        collapsed={collapsed}
        onToggle={toggleSidebar}
      />
      <div className="flex min-w-0 flex-1 flex-col">
        <Topbar workspace={workspace} />
        <main
          id="main"
          tabIndex={-1}
          data-testid="main"
          className="lp-pane min-h-0 flex-1 outline-none"
        >
          <motion.div
            key={`${ws ?? ""}:${section}`}
            variants={fadeRise}
            initial="hidden"
            animate="visible"
            className="h-full"
          >
            <Outlet />
          </motion.div>
        </main>
      </div>
      <div
        role="status"
        aria-live="polite"
        className="sr-only"
        data-testid="route-announcer"
      >
        {announcement}
      </div>
    </div>
  )
}
