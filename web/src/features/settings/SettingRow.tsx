import type { ReactNode } from "react"

import { Label } from "@/components/ui/label"

type SettingRowProps = {
  label: string
  description?: string
  htmlFor?: string
  children: ReactNode
}

/** One preference: label and help text on the leading side, the control on the trailing side. */
export function SettingRow({
  label,
  description,
  htmlFor,
  children,
}: SettingRowProps) {
  return (
    <div className="flex items-center justify-between gap-6 py-3">
      <div className="flex min-w-0 flex-col gap-0.5">
        {htmlFor ? (
          <Label htmlFor={htmlFor} className="text-[13px] font-medium">
            {label}
          </Label>
        ) : (
          <span className="text-[13px] font-medium">{label}</span>
        )}
        {description ? (
          <span className="text-[12px] text-muted-foreground">
            {description}
          </span>
        ) : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  )
}
