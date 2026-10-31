import { ShuffleIcon } from "lucide-react"

import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"

import { REVIEW_COPY } from "./copy"

export type ReviewControlsValue = {
  limit: number
  shuffle: boolean
}

type ReviewControlsProps = {
  value: ReviewControlsValue
  onChange: (next: ReviewControlsValue) => void
}

const LIMITS = [10, 20, 30, 50, 100]

export function ReviewControls({ value, onChange }: ReviewControlsProps) {
  return (
    <div className="flex flex-col gap-4 sm:flex-row sm:items-end">
      <div className="flex flex-col gap-1.5">
        <Label
          htmlFor="review-limit"
          className="text-[12px] text-muted-foreground"
        >
          {REVIEW_COPY.limitLabel}
        </Label>
        <Select
          value={String(value.limit)}
          onValueChange={(v) => onChange({ ...value, limit: Number(v) })}
        >
          <SelectTrigger
            id="review-limit"
            data-testid="review-limit"
            className="w-32"
          >
            <SelectValue dir="auto" />
          </SelectTrigger>
          <SelectContent>
            {LIMITS.map((n) => (
              <SelectItem key={n} value={String(n)}>
                {n} cards
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="flex items-center gap-2 pb-1.5">
        <Switch
          id="review-shuffle"
          data-testid="review-shuffle"
          checked={value.shuffle}
          onCheckedChange={(v) => onChange({ ...value, shuffle: v })}
        />
        <Label
          htmlFor="review-shuffle"
          className="flex items-center gap-1.5 text-[13px]"
        >
          <ShuffleIcon className="size-3.5" />
          {REVIEW_COPY.shuffleLabel}
        </Label>
      </div>
    </div>
  )
}
