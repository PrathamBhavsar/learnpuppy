import { InfoTip } from "@/components/app/info/info"
import { CheckIcon, CopyIcon } from "lucide-react"
import { useEffect, useRef, useState } from "react"

import type { AccountDetail } from "@/api/pending-types"
import { useSetAccountDisabled } from "@/api/queries/accounts"
import { CountUp } from "@/components/app/count-up"
import { Switch } from "@/components/ui/switch"
import { toastError } from "@/lib/toast"
import { cn } from "@/lib/utils"

import { CooldownRing } from "./CooldownRing"
import {
  STATUS_LABEL,
  formatTokens,
  providerTitle,
  reauthCommand,
} from "./model"

import { ACCOUNTS_INFO } from "./help"

import "./accounts.css"

const DOT: Record<AccountDetail["status"], string> = {
  ready: "bg-success",
  cooldown: "bg-warning",
  auth_expired: "bg-destructive",
  disabled: "bg-muted-foreground",
}

type AccountCardProps = { account: AccountDetail; now: number }

export function AccountCard({ account, now }: AccountCardProps) {
  const setDisabled = useSetAccountDisabled()
  const [copied, setCopied] = useState(false)
  const [pulse, setPulse] = useState(false)
  const previous = useRef(account.status)
  const command = reauthCommand(account)
  const id = account.id

  // A rate-limit event (status turning to cooldown) pulses the border once.
  useEffect(() => {
    const was = previous.current
    previous.current = account.status
    if (was === account.status || account.status !== "cooldown") return
    setPulse(true)
    const timer = window.setTimeout(() => setPulse(false), 1000)
    return () => window.clearTimeout(timer)
  }, [account.status])

  const copy = async () => {
    if (!command) return
    try {
      await navigator.clipboard.writeText(command)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    } catch (error) {
      toastError(error, "Could not copy the command")
    }
  }

  return (
    <li
      data-testid={`account-${id}`}
      data-status={account.status}
      data-pulse={pulse}
      className={cn(
        "flex flex-col gap-3 border bg-card p-4 text-card-foreground",
        pulse && "lp-border-pulse"
      )}
    >
      <div className="flex items-start gap-3">
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <span
            className="truncate text-[13px] font-medium"
            data-testid={`account-email-${id}`}
          >
            {account.email ?? `${providerTitle(account.provider)} account`}
          </span>
          <div className="flex items-center gap-2 text-[12px] text-muted-foreground">
            {account.tier ? (
              <span data-testid={`account-tier-${id}`} className="uppercase">
                {account.tier.replace(/_/g, " ")}
              </span>
            ) : null}
            <span className="flex items-center gap-1.5">
              <span
                aria-hidden
                className={cn(
                  "lp-status-dot size-2 rounded-full",
                  DOT[account.status]
                )}
              />
              <span data-testid={`account-status-${id}`}>
                {STATUS_LABEL[account.status]}
              </span>
              <InfoTip
                info={
                  account.status === "cooldown"
                    ? ACCOUNTS_INFO.cooldown
                    : ACCOUNTS_INFO.status
                }
                testId={`info-status-${id}`}
              />
            </span>
          </div>
        </div>
        {account.status === "cooldown" && account.cooldownUntil ? (
          <CooldownRing
            until={account.cooldownUntil}
            now={now}
            testId={`account-ring-${id}`}
          />
        ) : null}
        <label className="flex items-center gap-2 text-[12px]">
          <span className="text-muted-foreground">
            {account.status === "disabled" ? "Enable" : "Disable"}
          </span>
          <Switch
            size="sm"
            checked={account.status !== "disabled"}
            disabled={setDisabled.isPending}
            data-testid={`account-toggle-${id}`}
            aria-label={`${account.status === "disabled" ? "Enable" : "Disable"} ${account.email ?? id}`}
            onCheckedChange={(on) =>
              setDisabled.mutate(
                { id, disabled: !on },
                {
                  onError: (error) =>
                    toastError(error, "Could not update the account"),
                }
              )
            }
          />
        </label>
      </div>

      <dl className="grid grid-cols-3 gap-2 text-[12px]">
        <div className="flex flex-col">
          <dt className="flex items-center gap-1 text-muted-foreground">
            Requests
            <InfoTip info={ACCOUNTS_INFO.usage} testId={`info-usage-${id}`} />
          </dt>
          <dd className="font-medium">
            <CountUp value={account.requests} />
          </dd>
        </div>
        <div className="flex flex-col">
          <dt className="text-muted-foreground">Tokens in</dt>
          <dd className="font-medium">
            <CountUp value={account.tokensIn} format={formatTokens} />
          </dd>
        </div>
        <div className="flex flex-col">
          <dt className="text-muted-foreground">Tokens out</dt>
          <dd className="font-medium">
            <CountUp value={account.tokensOut} format={formatTokens} />
          </dd>
        </div>
      </dl>

      {account.status === "cooldown" && account.cooldownReason ? (
        <p className="text-[12px] text-muted-foreground">
          {account.cooldownReason}
        </p>
      ) : null}

      {command ? (
        <div
          data-testid={`account-reauth-${id}`}
          className="flex items-center gap-2 border bg-muted/40 px-2 py-1.5"
        >
          <span className="text-[11px] text-muted-foreground">Re-auth</span>
          <code className="min-w-0 flex-1 truncate text-[12px]" title={command}>
            {command}
          </code>
          <button
            type="button"
            onClick={() => void copy()}
            aria-label="Copy re-auth command"
            data-testid={`account-reauth-copy-${id}`}
            className="grid size-6 place-items-center text-muted-foreground hover:text-foreground"
          >
            {copied ? (
              <CheckIcon className="size-3.5" />
            ) : (
              <CopyIcon className="size-3.5" />
            )}
          </button>
        </div>
      ) : null}
    </li>
  )
}
