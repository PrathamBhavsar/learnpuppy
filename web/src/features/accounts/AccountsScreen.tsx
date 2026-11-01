import { InfoTip, ScreenGuide } from "@/components/app/info/info"
import { PlusIcon, RefreshCwIcon, UsersRoundIcon } from "lucide-react"
import { useState } from "react"

import { toApiError } from "@/api/errors"
import type { RoleProfile } from "@/api/pending-types"
import { useAccounts, useRescanAccounts } from "@/api/queries/accounts"
import {
  useSaveSettings,
  useSettings,
  useSettingsCatalog,
} from "@/api/queries/settings"
import { EmptyState } from "@/components/app/empty-state"
import { ErrorState } from "@/components/app/error-state"
import { PageHeader } from "@/components/app/page-header"
import { RoleSlots } from "@/components/app/role-slots/RoleSlots"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { usePageTitle } from "@/hooks/use-page-title"
import { toastError } from "@/lib/toast"

import { AccountCard } from "./AccountCard"
import { AddProfileDialog } from "./AddProfileDialog"
import { groupAccounts, readyCount } from "./model"
import { ACCOUNTS_GUIDE_PARTS, ACCOUNTS_INFO } from "./help"
import { useNow } from "./use-now"

function RescanButton() {
  const rescan = useRescanAccounts()
  return (
    <Button
      variant="outline"
      size="sm"
      disabled={rescan.isPending}
      data-testid="accounts-rescan"
      onClick={() =>
        rescan.mutate(undefined, {
          onError: (error) => toastError(error, "Rescan failed"),
        })
      }
    >
      {rescan.isPending ? (
        <Spinner data-icon="inline-start" />
      ) : (
        <RefreshCwIcon data-icon="inline-start" />
      )}
      Rescan
    </Button>
  )
}

function RoleSlotsSection() {
  const settings = useSettings()
  const catalog = useSettingsCatalog()
  const save = useSaveSettings()

  if (settings.isError || catalog.isError) {
    const error = toApiError(settings.error ?? catalog.error)
    return (
      <ErrorState
        title="Role slots did not load"
        message={error.message}
        requestId={error.requestId}
        onRetry={() => Promise.all([settings.refetch(), catalog.refetch()])}
        className="border py-8"
      />
    )
  }
  if (!settings.data || !catalog.data)
    return (
      <SkeletonBlock className="h-64 w-full" data-testid="role-slots-loading" />
    )

  const doc = settings.data
  return (
    <RoleSlots
      testIdPrefix="global"
      catalog={catalog.data}
      saved={doc.profiles}
      save={(profiles: Record<string, RoleProfile>) =>
        save.mutateAsync({ ...doc, profiles })
      }
    />
  )
}

export function AccountsScreen() {
  usePageTitle("Accounts")
  const accounts = useAccounts()
  const [adding, setAdding] = useState(false)
  const cooling = accounts.data?.some((a) => a.status === "cooldown") ?? false
  const now = useNow(cooling)

  const list = accounts.data
  const ready = list ? readyCount(list) : 0

  return (
    <section
      data-testid="screen-accounts"
      aria-busy={accounts.isPending}
      className="mx-auto flex min-h-full w-full max-w-5xl flex-col gap-8 p-6"
    >
      <PageHeader
        title="Accounts"
        description="Model accounts found on this machine and the roles they fill."
        actions={
          <>
            <ScreenGuide
              title="Accounts"
              summary="The AI logins this app borrows from your computer, and which one does which job."
              parts={ACCOUNTS_GUIDE_PARTS}
            />
            <RescanButton />
            <Button
              size="sm"
              data-testid="accounts-add"
              onClick={() => setAdding(true)}
            >
              <PlusIcon data-icon="inline-start" />
              Add Claude profile
            </Button>
          </>
        }
      />

      {accounts.isPending ? (
        <div
          data-testid="accounts-loading"
          className="grid gap-3 sm:grid-cols-2"
        >
          {[0, 1, 2, 3].map((i) => (
            <SkeletonBlock key={i} className="h-36 w-full" />
          ))}
        </div>
      ) : accounts.isError ? (
        <ErrorState
          title="Accounts did not load"
          message={toApiError(accounts.error).message}
          requestId={toApiError(accounts.error).requestId}
          onRetry={() => accounts.refetch()}
          className="border"
        />
      ) : list && list.length === 0 ? (
        <EmptyState
          icon={UsersRoundIcon}
          title="No accounts found"
          description="Sign in with a CLI on this machine (run claude login, codex login or agy auth login), then rescan."
          action={<RescanButton />}
          className="border"
        />
      ) : list ? (
        <>
          {ready === 0 ? (
            <Alert variant="destructive" data-testid="accounts-none-ready">
              <AlertTitle>No account is ready</AlertTitle>
              <AlertDescription>
                Chats and ingest cannot run until one account is ready. Wait out
                a cooldown, re-enable a disabled account, or sign in again with
                the command shown on its card, then rescan.
              </AlertDescription>
            </Alert>
          ) : null}
          <div className="flex flex-col gap-6" data-testid="accounts-list">
            {groupAccounts(list).map((group) => (
              <section
                key={group.provider}
                aria-labelledby={`group-${group.provider}`}
                data-testid={`accounts-group-${group.provider}`}
                className="flex flex-col gap-2"
              >
                <h2
                  id={`group-${group.provider}`}
                  className="font-heading text-[14px] font-medium"
                >
                  {group.title}{" "}
                  <span className="text-muted-foreground tabular-nums">
                    {group.accounts.length}
                  </span>
                </h2>
                <ul role="list" className="grid gap-3 sm:grid-cols-2">
                  {group.accounts.map((account) => (
                    <AccountCard key={account.id} account={account} now={now} />
                  ))}
                </ul>
              </section>
            ))}
          </div>
        </>
      ) : null}

      <section
        aria-labelledby="role-slots-heading"
        className="flex flex-col gap-3"
      >
        <div className="flex flex-col gap-1">
          <div className="flex items-center gap-1.5">
            <h2
              id="role-slots-heading"
              className="font-heading text-[16px] font-medium"
            >
              Role slots
            </h2>
            <InfoTip info={ACCOUNTS_INFO.roleSlots} />
          </div>
          <p className="text-[13px] text-muted-foreground">
            Which provider, model and effort each agent role uses. Workspaces
            can override these in their own settings.
          </p>
        </div>
        <RoleSlotsSection />
      </section>

      <AddProfileDialog open={adding} onOpenChange={setAdding} />
    </section>
  )
}
