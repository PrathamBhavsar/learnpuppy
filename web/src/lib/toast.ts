import { toast } from "sonner"

import { toApiError } from "@/api/errors"

/** Short success toast (3 s, docs/web/06 §Cross-cutting). */
export function toastSuccess(message: string): void {
  toast.success(message, { duration: 3000 })
}

export function toastInfo(message: string, description?: string): void {
  toast(message, { description, duration: 4000 })
}

async function copy(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    return false
  }
}

/** Error toast that persists and carries a "Copy details" action with the request id. */
export function toastError(error: unknown, title?: string): void {
  const api = toApiError(error)
  toast.error(title ?? api.message, {
    description: title ? api.message : undefined,
    duration: Infinity,
    closeButton: true,
    action: {
      label: "Copy details",
      onClick: () => {
        void copy(api.describe()).then((ok) => {
          if (ok) toast.success("Details copied", { duration: 2000 })
        })
      },
    },
  })
}
