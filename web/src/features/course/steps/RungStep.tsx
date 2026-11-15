import { PromptMarkdown } from "@/components/app/prompt-markdown"

/** The worked example at the rung the server picked; shown as given, never re-levelled here. */
export function RungStep({ text }: { text: string }) {
  return (
    <aside data-testid="step-rung" className="border-s-2 ps-3">
      <PromptMarkdown source={text} />
    </aside>
  )
}
