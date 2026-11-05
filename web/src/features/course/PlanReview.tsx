import type { CoursePlan } from "@/api/method-types"
import { Button } from "@/components/ui/button"

/** The lesson map: each lesson with what it builds on. Confirming is the server's gate to teaching. */
export function PlanReview({
  plan,
  busy,
  onConfirm,
}: {
  plan: CoursePlan
  busy: boolean
  onConfirm: () => void
}) {
  const title = (id: string) =>
    plan.lessons.find((l) => l.id === id)?.title ?? id
  return (
    <section data-testid="course-plan" className="flex flex-col gap-3">
      <h2 className="font-heading text-[18px] font-medium">Your course plan</h2>
      <ol className="flex flex-col gap-2">
        {plan.lessons.map((l, n) => (
          <li key={l.id} className="border p-3 text-[13px]">
            <p className="font-medium">
              {n + 1}. {l.title}{" "}
              <span className="font-normal text-muted-foreground">
                ({l.kind})
              </span>
            </p>
            {l.dependsOn.length ? (
              <p className="text-muted-foreground">
                Builds on: {l.dependsOn.map(title).join(", ")}
              </p>
            ) : null}
          </li>
        ))}
      </ol>
      <Button className="self-start" disabled={busy} onClick={onConfirm}>
        Looks right, begin
      </Button>
    </section>
  )
}
