import { LazyMarkdown } from "../markdown/LazyMarkdown"
import { cn } from "@/lib/utils"

type Option = { key: string; text: string }

type OptionListProps = {
  tool: string
  options: Option[]
  picked: readonly string[]
  multiple: boolean
  disabled: boolean
  onToggle: (key: string) => void
  label: string
}

function letterOf(index: number): string {
  return String.fromCharCode(65 + index)
}

/** Lettered options as a radio group (or checkbox group when several may be picked). */
export function OptionList({
  tool,
  options,
  picked,
  multiple,
  disabled,
  onToggle,
  label,
}: OptionListProps) {
  return (
    <div
      role={multiple ? "group" : "radiogroup"}
      aria-label={label}
      className="flex flex-col gap-1.5"
    >
      {options.map((option, index) => {
        const letter = letterOf(index)
        const checked = picked.includes(option.key)
        return (
          <button
            key={option.key}
            type="button"
            role={multiple ? "checkbox" : "radio"}
            aria-checked={checked}
            disabled={disabled}
            onClick={() => onToggle(option.key)}
            data-testid={`${tool}-option-${letter}`}
            data-checked={checked}
            style={
              {
                "--lp-delay": `${Math.min(index, 7) * 24}ms`,
              } as React.CSSProperties
            }
            className={cn(
              "lp-rise flex items-start gap-2.5 border px-3 py-2 text-start text-[13px] transition-colors outline-none focus-visible:ring-1 focus-visible:ring-ring/50 active:translate-y-px",
              checked
                ? "border-primary bg-primary/10"
                : "hover:border-primary/60 hover:bg-muted/50"
            )}
          >
            <span
              aria-hidden
              className={cn(
                "mt-0.5 flex size-4 shrink-0 items-center justify-center border text-[10px] font-medium",
                multiple ? "" : "rounded-full",
                checked
                  ? "border-primary bg-primary text-primary-foreground"
                  : "text-muted-foreground"
              )}
            >
              {checked ? (
                <span
                  className={cn(
                    "block size-1.5 scale-100 bg-primary-foreground transition-transform",
                    multiple ? "" : "rounded-full"
                  )}
                />
              ) : (
                letter
              )}
            </span>
            <div dir="auto" className="min-w-0 flex-1">
              <span className="me-1.5 font-medium text-muted-foreground tabular-nums">
                {letter}.
              </span>
              <LazyMarkdown className="inline text-[13px] [&_p]:m-0 [&_p]:inline">
                {option.text}
              </LazyMarkdown>
            </div>
          </button>
        )
      })}
    </div>
  )
}
