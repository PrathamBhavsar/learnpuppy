import type { LucideIcon } from "lucide-react"
import { useEffect, useState } from "react"

import { EmptyState } from "@/components/app/empty-state"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonRows } from "@/components/app/skeleton-block"
import { Button } from "@/components/ui/button"
import { usePageTitle } from "@/hooks/use-page-title"

type PlaceholderScreenProps = {
  title: string
  description: string
  icon: LucideIcon
  emptyTitle: string
  emptyDescription: string
  actionLabel?: string
  workspace?: string
  testId: string
}

/** Stand-in for a screen whose feature lands later: loading skeleton, then its empty state. */
export function PlaceholderScreen({
  title,
  description,
  icon,
  emptyTitle,
  emptyDescription,
  actionLabel,
  workspace,
  testId,
}: PlaceholderScreenProps) {
  const [loading, setLoading] = useState(true)
  usePageTitle(title, workspace)

  useEffect(() => {
    const timer = window.setTimeout(() => setLoading(false), 450)
    return () => window.clearTimeout(timer)
  }, [])

  return (
    <section
      data-testid={testId}
      aria-busy={loading}
      className="flex min-h-full flex-col gap-6 p-6"
    >
      <PageHeader title={title} description={description} />
      {loading ? (
        <SkeletonRows rows={6} className="max-w-3xl" />
      ) : (
        <EmptyState
          icon={icon}
          title={emptyTitle}
          description={emptyDescription}
          action={
            actionLabel ? (
              <Button variant="outline">{actionLabel}</Button>
            ) : undefined
          }
          className="flex-1"
        />
      )}
    </section>
  )
}
