import { createFileRoute } from "@tanstack/react-router"

import { AppShell } from "@/features/shell/AppShell"

export const Route = createFileRoute("/_shell")({ component: AppShell })
