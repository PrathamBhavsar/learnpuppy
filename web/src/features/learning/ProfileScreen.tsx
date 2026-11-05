import { PlusIcon } from "lucide-react"
import { useState } from "react"

import { toApiError } from "@/api/errors"
import { ErrorState } from "@/components/app/error-state"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { usePageTitle } from "@/hooks/use-page-title"
import { useWorkspaceName } from "@/hooks/use-workspace"
import { toastError, toastSuccess } from "@/lib/toast"

import { useLearningActions, useProfile } from "./api"

/** F34: dated profile entries for edges, outcomes and preferences; the latest per key wins. */
export function ProfileScreen({ ws }: { ws: string }) {
  usePageTitle("Profile", useWorkspaceName(ws))
  const profile = useProfile(ws)
  const { setProfile } = useLearningActions(ws)
  const [key, setKey] = useState("")
  const [value, setValue] = useState("")
  const valid = key.trim() !== "" && value.trim() !== ""

  if (profile.isPending) {
    return (
      <div className="flex flex-col gap-4 p-6">
        <SkeletonBlock className="h-8 w-48" />
        <SkeletonBlock className="h-40 w-full" />
      </div>
    )
  }
  if (profile.isError) {
    return (
      <div className="p-6">
        <ErrorState
          message={toApiError(profile.error).message}
          onRetry={() => profile.refetch()}
        />
      </div>
    )
  }

  const entries = [...(profile.data?.entries ?? [])].sort(
    (a, b) => b.ts - a.ts
  )

  const add = () => {
    if (!valid) return
    setProfile.mutate(
      { key: key.trim(), value: value.trim() },
      {
        onSuccess: () => {
          toastSuccess("Profile entry saved")
          setKey("")
          setValue("")
        },
        onError: (error) => toastError(error, "Could not save entry"),
      }
    )
  }

  return (
    <div className="flex flex-col gap-6 p-6" data-testid="profile-screen">
      <PageHeader
        title="Profile"
        description="Dated entries for edges, outcomes and preferences. A new entry supersedes the old one for that key, which stays in the log."
      />
      <Card>
        <CardHeader>
          <CardTitle className="text-[14px]">Add an entry</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <div className="flex flex-col gap-1">
            <Label htmlFor="profile-key">Key</Label>
            <Input
              id="profile-key"
              className="w-64"
              placeholder="e.g. edge:fast-typist"
              value={key}
              onChange={(e) => setKey(e.target.value)}
            />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="profile-value">Value</Label>
            <Input
              id="profile-value"
              value={value}
              onChange={(e) => setValue(e.target.value)}
            />
          </div>
          <Button
            className="self-start"
            disabled={!valid || setProfile.isPending}
            onClick={add}
          >
            <PlusIcon className="size-4" />
            Add entry
          </Button>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle className="text-[14px]">Current entries</CardTitle>
        </CardHeader>
        <CardContent>
          {entries.length ? (
            <ul className="flex flex-col gap-2">
              {entries.map((e) => (
                <li key={e.key} className="flex flex-col text-[13px]">
                  <span className="font-medium">{e.key}</span>
                  <span className="text-muted-foreground">{e.value}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-[13px] text-muted-foreground">
              No profile entries yet.
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
