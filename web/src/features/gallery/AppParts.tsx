import { InboxIcon } from "lucide-react"
import { useState } from "react"

import { CountUp } from "@/components/app/count-up"
import { EmptyState } from "@/components/app/empty-state"
import { ErrorState } from "@/components/app/error-state"
import { JobsPopover } from "@/components/app/jobs-popover"
import { Kbd } from "@/components/app/kbd"
import { MasteryRing } from "@/components/app/mastery-ring"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonBlock, SkeletonRows } from "@/components/app/skeleton-block"
import { Button } from "@/components/ui/button"
import { dur, ease, spring } from "@/lib/motion"

import { Section, Specimen } from "./Section"

const SAMPLE_JOBS = [
  { id: "j1", label: "Research: lifetimes", detail: "Running for 42 s" },
  { id: "j2", label: "Ingest", detail: "Scanning 12 of 48 sessions" },
] as const

export function AppParts() {
  const [count, setCount] = useState(128)
  return (
    <>
      <Section id="app" title="App components">
        <Specimen label="CountUp (tabular numerals)">
          <span
            className="text-[24px] font-medium"
            data-testid="gallery-countup"
          >
            <CountUp value={count} />
          </span>
          <Button
            size="sm"
            variant="outline"
            onClick={() => setCount((c) => c + 37)}
            data-testid="gallery-countup-add"
          >
            Add 37
          </Button>
        </Specimen>
        <Specimen label="MasteryRing 0 / .25 / .5 / .75 / 1">
          {[0, 0.25, 0.5, 0.75, 1].map((v) => (
            <MasteryRing key={v} value={v} size={32} />
          ))}
        </Specimen>
        <Specimen label="Kbd">
          <Kbd combo="mod+k" />
          <Kbd combo="shift+enter" />
          <Kbd combo="[" />
        </Specimen>
        <Specimen
          label="Skeleton shimmer"
          className="w-64 flex-col items-stretch"
        >
          <SkeletonBlock className="h-4 w-full" />
          <SkeletonBlock className="h-4 w-2/3" />
          <SkeletonRows rows={2} />
        </Specimen>
        <Specimen label="Jobs popover">
          <JobsPopover jobs={SAMPLE_JOBS} />
          <JobsPopover jobs={[]} />
        </Specimen>
        <Specimen label="Page header" className="w-96">
          <PageHeader
            title="Digest"
            description="What your agents decided."
            actions={<Button size="sm">Refresh</Button>}
          />
        </Specimen>
      </Section>
      <Section
        id="states"
        title="Empty and error states"
        className="[&>*]:min-w-72 [&>*]:flex-1"
      >
        <Specimen label="EmptyState">
          <EmptyState
            icon={InboxIcon}
            title="Nothing to review"
            description="Cards appear here after an ingest."
            action={<Button variant="outline">Go to ingest</Button>}
            focusAction={false}
            className="w-full border py-10"
          />
        </Specimen>
        <Specimen label="ErrorState">
          <ErrorState
            message="The server did not answer."
            requestId="req_9f2a41"
            onRetry={() => new Promise((resolve) => setTimeout(resolve, 800))}
            className="w-full border py-10"
          />
        </Specimen>
      </Section>
      <Section id="motion" title="Motion tokens">
        <Specimen label="Durations (s)" className="flex-col items-start">
          {Object.entries(dur).map(([name, value]) => (
            <code key={name} className="text-[12px] tabular-nums">
              {name} {value}
            </code>
          ))}
        </Specimen>
        <Specimen label="Easings" className="flex-col items-start">
          {Object.entries(ease).map(([name, value]) => (
            <code key={name} className="text-[12px] tabular-nums">
              {name} cubic-bezier({value.join(", ")})
            </code>
          ))}
        </Specimen>
        <Specimen label="Springs" className="flex-col items-start">
          {Object.entries(spring).map(([name, value]) => (
            <code key={name} className="text-[12px] tabular-nums">
              {name} k={value.stiffness} c={value.damping}
            </code>
          ))}
        </Specimen>
      </Section>
    </>
  )
}
