import { Button } from "@/components/ui/button"

/** A short rest after a lesson; the server records that it was taken. */
export function BreakPrompt({
  title,
  busy,
  onRested,
}: {
  title: string
  busy: boolean
  onRested: () => void
}) {
  return (
    <section data-testid="step-break" className="flex flex-col gap-3">
      <h2 className="font-heading text-[16px] font-medium">
        Take a short break
      </h2>
      <p className="text-[13px]">
        You finished {title}. Stand up, look away from the screen, then come
        back.
      </p>
      <Button className="self-start" disabled={busy} onClick={onRested}>
        I&apos;m back
      </Button>
    </section>
  )
}
