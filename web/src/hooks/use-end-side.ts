import { useDirection } from "@/components/ui/direction"

/** Physical Radix `side` that points toward the inline-end edge (right in LTR, left in RTL). */
export function useEndSide(): "left" | "right" {
  return useDirection() === "rtl" ? "left" : "right"
}
