import { describe, expect, it } from "vitest"

import type { AccountDetail } from "@/api/pending-types"

import { buildDiagnostics, redactSecrets } from "./diagnostics"

const TOKEN = "sk-ant-oat01-PLANTEDSECRET1234567890abcdef"

describe("redactSecrets", () => {
  it("removes provider keys, bearer tokens, JWTs and key=value secrets", () => {
    const text = [
      `key ${TOKEN}`,
      "Authorization: Bearer abcdefghijklmnop12345",
      "jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.sig",
      "refresh_token=abc123def456ghi789",
      "gh ghp_abcdefghijklmnopqrstuvwxyz0123456789",
    ].join("\n")
    const out = redactSecrets(text)
    for (const leak of [
      "PLANTEDSECRET",
      "abcdefghijklmnop12345",
      "eyJzdWIi",
      "abc123def456ghi789",
      "ghp_abcdefghijkl",
    ])
      expect(out).not.toContain(leak)
    expect(out).toContain("[redacted]")
  })

  it("leaves ordinary diagnostics alone", () => {
    const text = "version: 0.1.0\nengine: faux\nclaude: 2 (1 ready, 1 cooldown)"
    expect(redactSecrets(text)).toBe(text)
  })
})

describe("buildDiagnostics", () => {
  const account = (over: Partial<AccountDetail>): AccountDetail => ({
    id: "claude:1",
    provider: "claude",
    email: "person@example.com",
    status: "ready",
    requests: 1,
    tokensIn: 1,
    tokensOut: 1,
    howToConnect: "claude login",
    models: [],
    ...over,
  })

  it("summarises accounts without emails and strips a planted token from any field", () => {
    const text = buildDiagnostics({
      version: "1.2.3",
      engine: "real",
      dataDir: `/Users/x/.learnpuppy token=${TOKEN}`,
      accounts: [
        account({ cooldownReason: TOKEN }),
        account({ id: "claude:2", status: "cooldown" }),
      ],
      settings: undefined,
      userAgent: "test",
    })
    expect(text).toContain("claude: 2 (1 ready, 1 cooldown)")
    expect(text).not.toContain("person@example.com")
    expect(text).not.toContain("PLANTEDSECRET")
  })
})
