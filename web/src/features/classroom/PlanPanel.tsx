import { WorkflowIcon } from "lucide-react"
import { Suspense, lazy } from "react"

import type { ClassroomJob, ClassroomView } from "@/api/pending-types"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

import { JobsPanel } from "./JobsPanel"
import { PhaseStepper } from "./PhaseStepper"

const PlanDag = lazy(() => import("./PlanDag"))

type PlanPanelProps = {
  ws: string
  view: ClassroomView
  onOpenArtifact: (job: ClassroomJob) => void
}

/** Right pane: phase stepper above Plan / Jobs tabs. Read-only; the server owns every status. */
export function PlanPanel({ ws, view, onOpenArtifact }: PlanPanelProps) {
  const running = view.jobs.filter((j) => j.status === "running").length
  return (
    <div className="flex size-full min-h-0 flex-col" data-testid="plan-panel">
      <div className="border-b p-3">
        <PhaseStepper
          kind={view.kind}
          phase={view.phase}
          probe={view.probeProgress}
        />
      </div>
      <Tabs defaultValue="plan" className="min-h-0 flex-1 gap-0">
        <TabsList className="m-2 self-start">
          <TabsTrigger value="plan" data-testid="tab-plan">
            Plan
          </TabsTrigger>
          <TabsTrigger value="jobs" data-testid="tab-jobs">
            Jobs
            {view.jobs.length > 0 ? (
              <Badge variant="secondary" className="ms-1 h-4 px-1 tabular-nums">
                {running > 0 ? running : view.jobs.length}
              </Badge>
            ) : null}
          </TabsTrigger>
        </TabsList>
        <TabsContent value="plan" className="min-h-0 flex-1">
          {view.plan && view.plan.nodes.length > 0 ? (
            <Suspense fallback={<SkeletonBlock className="m-3 h-48" />}>
              <PlanDag plan={view.plan} />
            </Suspense>
          ) : (
            <div
              data-testid="plan-empty"
              className="flex flex-col items-center gap-2 px-4 py-10 text-center"
            >
              <WorkflowIcon
                aria-hidden
                className="size-6 text-muted-foreground"
                strokeWidth={1.5}
              />
              <p className="text-[13px] text-muted-foreground">
                {view.phase === "probe"
                  ? "The plan appears once the probe questions are done."
                  : "No plan for this chat."}
              </p>
            </div>
          )}
        </TabsContent>
        <TabsContent value="jobs" className="min-h-0 flex-1 overflow-auto">
          <JobsPanel ws={ws} jobs={view.jobs} onOpenArtifact={onOpenArtifact} />
        </TabsContent>
      </Tabs>
    </div>
  )
}
