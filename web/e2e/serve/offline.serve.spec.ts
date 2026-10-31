import { expect, test } from "@playwright/test"

// Real server: kill the event stream and the API, confirm the banner and silent recovery.
const WS = "ws_5eed0001"

test("aborting the SSE stream shows the banner, restoring it recovers", async ({
  page,
}) => {
  await page.route(
    /\/api\/v1\/(workspaces|accounts)(\?.*)?$/,
    async (route) => {
      const response = await route.fetch()
      const json = (await response.json()) as { items?: unknown }
      await route.fulfill({ response, json: json.items ?? json })
    }
  )
  await page.goto(`/w/${WS}/learn`)
  await expect(page.getByTestId("chat-item-chat_teach")).toBeVisible()
  await page.route("**/api/v1/events*", (r) => r.abort())
  await page.evaluate(() => window.dispatchEvent(new Event("offline")))
  await page.reload()
  await expect(page.getByTestId("connection-banner")).toBeVisible({
    timeout: 10_000,
  })
  await page.unroute("**/api/v1/events*")
  await expect(page.getByTestId("connection-banner")).toContainText(
    "Back online",
    { timeout: 30_000 }
  )
})
