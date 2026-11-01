import type { ChatKind, ChatPhase } from "@/api/pending-types"
import { InfoTip } from "@/components/app/info/info"
import { cn } from "@/lib/utils"

import { ProbeProgress } from "./dock/ProbeProgress"
import { classroomInfo } from "./info"

type PhaseStepperProps = {
  kind: ChatKind
  phase: ChatPhase
  probe: { asked: number; max: number }
}

const RANK: Record<ChatPhase, number> = {
  probe: 0,
  plan: 1,
  teach: 2,
  practice: 2,
  done: 3,
}

const RADIUS = 9
const CIRCUMFERENCE = 2 * Math.PI * RADIUS

/** Probe, Plan, then Teach (or Practice). Read-only: the active ring fills, finished steps draw a check. */
export function PhaseStepper({ kind, phase, probe }: PhaseStepperProps) {
  const steps = [
    { key: "probe", label: "Probe" },
    { key: "plan", label: "Plan" },
    {
      key: kind === "practice" ? "practice" : "teach",
      label: kind === "practice" ? "Practice" : "Teach",
    },
  ]
  const active = RANK[phase]
  return (
    <div className="flex flex-col gap-2" data-testid="phase-stepper">
      <div className="flex items-center gap-2">
        <ol
          className="flex flex-1 items-center gap-2"
          aria-label="Lesson phases"
        >
          {steps.map((step, index) => {
            const state =
              index < active ? "done" : index === active ? "active" : "todo"
            return (
              <li
                key={step.key}
                data-testid={`phase-${step.key}`}
                data-state={state}
                aria-current={state === "active" ? "step" : undefined}
                className="flex flex-1 items-center gap-2 last:flex-none"
              >
                <svg
                  aria-hidden
                  viewBox="0 0 24 24"
                  className="size-6 shrink-0"
                >
                  <circle
                    cx="12"
                    cy="12"
                    r={RADIUS}
                    fill="none"
                    strokeWidth="2"
                    className="stroke-border"
                  />
                  {state === "active" ? (
                    <circle
                      cx="12"
                      cy="12"
                      r={RADIUS}
                      fill="none"
                      strokeWidth="2"
                      strokeLinecap="butt"
                      strokeDasharray={CIRCUMFERENCE}
                      strokeDashoffset={CIRCUMFERENCE * 0.35}
                      transform="rotate(-90 12 12)"
                      className="lp-ring stroke-primary"
                      style={
                        { "--lp-circ": CIRCUMFERENCE } as React.CSSProperties
                      }
                    />
                  ) : null}
                  {state === "done" ? (
                    <>
                      <circle
                        cx="12"
                        cy="12"
                        r={RADIUS}
                        fill="none"
                        strokeWidth="2"
                        className="stroke-success"
                      />
                      <path
                        pathLength={1}
                        className="lp-draw stroke-success"
                        d="M8 12.5l2.8 2.6L16 9.5"
                        fill="none"
                        strokeWidth="2"
                      />
                    </>
                  ) : null}
                </svg>
                <span
                  className={cn(
                    "text-[12px]",
                    state === "todo" ? "text-muted-foreground" : "font-medium"
                  )}
                >
                  {step.label}
                </span>
                {index < steps.length - 1 ? (
                  <span aria-hidden className="h-px flex-1 bg-border" />
                ) : null}
              </li>
            )
          })}
        </ol>
        <InfoTip info={classroomInfo.phases} />
      </div>
      {phase === "probe" ? (
        <div className="flex items-center gap-2">
          <ProbeProgress
            done={probe.asked}
            max={probe.max}
            className="flex-1"
          />
          <span className="text-[11px] text-muted-foreground tabular-nums">
            {probe.asked}/{probe.max}
          </span>
        </div>
      ) : null}
    </div>
  )
}
