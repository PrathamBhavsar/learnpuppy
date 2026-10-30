import { Skeleton } from "@/components/ui/skeleton"
import { cn } from "@/lib/utils"

/** Skeleton with the 1.6 s shimmer sweep (static block under reduced motion). Size it to the content it stands for. */
export function SkeletonBlock({
  className,
  ...props
}: React.ComponentProps<typeof Skeleton>) {
  return (
    <Skeleton
      className={cn("lp-shimmer animate-none", className)}
      aria-hidden
      {...props}
    />
  )
}

/** Loading placeholder for a list of rows, sized like `rows` text lines. */
export function SkeletonRows({
  rows = 4,
  className,
}: {
  rows?: number
  className?: string
}) {
  return (
    <div
      role="status"
      aria-label="Loading"
      className={cn("flex flex-col gap-3", className)}
    >
      {Array.from({ length: rows }, (_, i) => (
        <SkeletonBlock
          key={i}
          className={i % 3 === 2 ? "h-4 w-2/3" : "h-4 w-full"}
        />
      ))}
    </div>
  )
}
