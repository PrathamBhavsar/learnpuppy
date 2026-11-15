import { Input } from "@/components/ui/input"

/** Options as radio buttons (mcq) or a text box (recall). The server grades; nothing is checked here. */
export function Choice({
  item,
  value,
  disabled,
  onChange,
}: {
  item: { id: string; kind: "mcq" | "recall"; options: string[] }
  value: string
  disabled?: boolean
  onChange: (text: string) => void
}) {
  if (item.kind === "recall")
    return (
      <Input
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        aria-label="Your answer"
        placeholder="Type your answer, or write: I don't know"
      />
    )
  return (
    <div
      role="radiogroup"
      aria-label="Options"
      className="flex flex-col gap-1.5"
    >
      {item.options.map((option) => (
        <label
          key={option}
          className="flex cursor-pointer items-start gap-2 border px-3 py-2 text-[13px] has-checked:border-primary has-checked:bg-primary/10"
        >
          <input
            type="radio"
            name={item.id}
            checked={value === option}
            disabled={disabled}
            onChange={() => onChange(option)}
          />
          {option}
        </label>
      ))}
    </div>
  )
}
