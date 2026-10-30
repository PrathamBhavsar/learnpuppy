import { Link, createFileRoute } from "@tanstack/react-router"
import { motion } from "motion/react"

import { DirectionToggle } from "@/components/app/direction-toggle"
import { PageHeader } from "@/components/app/page-header"
import { ThemeToggle } from "@/components/app/theme-toggle"
import { Badge } from "@/components/ui/badge"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { usePageTitle } from "@/hooks/use-page-title"
import { staggerDelay } from "@/lib/motion"
import { WorkspaceAvatar } from "@/features/shell/WorkspaceChip"
import { WORKSPACES } from "@/lib/static-workspaces"

function Home() {
  usePageTitle("Workspaces")
  return (
    <div
      className="lp-pane h-dvh w-dvw bg-background text-foreground"
      data-testid="home"
    >
      <div className="mx-auto flex max-w-5xl flex-col gap-6 p-8">
        <div className="flex items-start justify-between gap-4">
          <PageHeader
            title="Workspaces"
            description="Each workspace is its own knowledge base."
          />
          <div className="flex items-center gap-1">
            <DirectionToggle />
            <ThemeToggle />
          </div>
        </div>
        <ul
          className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3"
          role="list"
        >
          {WORKSPACES.map((w, i) => (
            <motion.li
              key={w.id}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{
                duration: 0.22,
                delay: staggerDelay(i, WORKSPACES.length),
              }}
            >
              <Link
                to="/w/$ws/graph"
                params={{ ws: w.id }}
                data-testid={`workspace-card-${w.id}`}
                className="block outline-none focus-visible:ring-1 focus-visible:ring-ring"
              >
                <Card className="transition-[border-color,transform] duration-[var(--dur-fast)] hover:-translate-y-0.5 hover:border-ring">
                  <CardHeader>
                    <div className="flex items-center gap-2">
                      <WorkspaceAvatar workspace={w} />
                      <CardTitle className="flex-1 truncate">
                        {w.name}
                      </CardTitle>
                      {w.due > 0 ? (
                        <Badge variant="secondary" className="tabular-nums">
                          {w.due} due
                        </Badge>
                      ) : null}
                    </div>
                    <CardDescription>
                      Open the graph, learn, review and digest for this
                      workspace.
                    </CardDescription>
                  </CardHeader>
                  <CardContent className="text-[11px] text-muted-foreground">
                    Static sample data
                  </CardContent>
                </Card>
              </Link>
            </motion.li>
          ))}
        </ul>
      </div>
    </div>
  )
}

export const Route = createFileRoute("/")({ component: Home })
