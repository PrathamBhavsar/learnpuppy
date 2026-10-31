/**
 * Request/response types for endpoints in docs/web/03 that `openapi/openapi.json` does not contain yet
 * (B20/B21/B22 add them). Everything here mirrors 03 exactly. When an endpoint lands in the generated
 * `schema.d.ts`, delete its entry from `PendingPaths` (and any type it made redundant) and point
 * `client.ts` at `paths`; the query hooks only import the named types below, so the swap is mechanical.
 */
import type { components } from "./schema"

type Schemas = components["schemas"]

export type ErrorCode = Schemas["ErrorCode"]
export type AccountSummary = Schemas["AccountSummaryDto"]
export type Job = Schemas["Job"]
export type IngestReport = Schemas["IngestReport"]

// ───────────────────────────── Workspaces ─────────────────────────────

export type WorkspaceSources = {
  claude: boolean
  codex: boolean
  agy: boolean
  git: boolean
  allow: string[]
  sinceDays: number | null
}

export type WorkspaceStats = {
  nodes: number
  chains: number
  chats: number
  dueCards: number
  lastActivityAt: number | null
}

/** `WorkspaceSummaryDto` plus what the list endpoint adds. */
export type WorkspaceListItem = Schemas["WorkspaceSummaryDto"] & {
  description: string
  stats: WorkspaceStats
  /** `workspace.json` is unreadable; only repair/reveal are offered. */
  damaged: boolean
}

export type RoleProfile = {
  provider: string
  model: string
  effort?: string | null
  /** Let the account pool fall back to other providers. */
  fallback?: boolean
}

export type WorkspaceDetail = WorkspaceListItem & {
  createdAt: number
  sources: WorkspaceSources
  overrides: { profiles: Record<string, RoleProfile> }
}

export type CreateWorkspaceBody = {
  name: string
  icon: string
  hue: string
  description?: string
  sources?: Partial<WorkspaceSources>
}

export type PatchWorkspaceBody = Partial<
  Pick<WorkspaceDetail, "name" | "icon" | "hue" | "description">
> & {
  sources?: Partial<WorkspaceSources>
  overrides?: { profiles: Record<string, RoleProfile> }
}

export type SearchHit = {
  workspaceId: string
  node: { id: string; text: string; kind: NodeKind }
  score: number
}

export type TrashItem = {
  id: string
  workspaceId: string
  name: string
  deletedAt: number
  sizeBytes: number
}

// ───────────────────────────── Accounts / settings ─────────────────────────────

export type AccountStatus = AccountSummary["status"]
/** `AccountDto`: never carries tokens or credentials. */
export type AccountDetail = AccountSummary & {
  requests: number
  tokensIn: number
  tokensOut: number
  /** The command that renews an expired login. */
  reauthHint?: string | null
}

export type UiSettings = {
  theme: "light" | "dark" | "system"
  direction: "ltr" | "rtl" | "auto"
  density: "comfortable" | "compact"
  reduceMotion: boolean
  confirmDestructive: boolean
  autoOpenBrowser: boolean
}

export type SettingsDoc = {
  /** Role → profile; roles without one are absent. */
  profiles: Record<string, RoleProfile>
  /** Role → `<provider>:auto` or an account id. */
  pins: Record<string, string>
  learningStyle: string
  /** Parallel model calls, 1..=6. */
  concurrencyCap: number
  ui: UiSettings
}

/** What the settings forms may offer. */
export type SettingsCatalog = {
  providers: { id: string; models: string[] }[]
  efforts: string[]
  roles: string[]
  pinRoles: string[]
  themes: string[]
  directions: string[]
  densities: string[]
}

/** `POST /settings/test`: a 1-token ping through the profile a role would use (docs/web/06 §Accounts). */
export type ProfilePingBody = {
  provider: string
  model: string
  effort?: string | null
}
export type ProfilePingResult = {
  ok: boolean
  latencyMs: number
  message: string
}

// ───────────────────────────── Graph ─────────────────────────────

export type NodeKind = "claim" | "concept" | "procedure" | "decision"
export type NodeSource = "claude" | "codex" | "agy" | "git" | "chat"
export type ChainKind = "supports" | "implies" | "contrasts" | "part_of"

export type GraphNode = {
  id: string
  text: string
  kind: NodeKind
  sessionId: string
  provenance: NodeSource
  createdAt: number
  degree: number
}

export type GraphChain = {
  id: string
  from: string
  to: string
  kind: ChainKind
  text?: string
}

export type GraphCluster = { id: string; label: string; nodeIds: string[] }

export type GraphSnapshot = {
  version: number
  nodes: GraphNode[]
  chains: GraphChain[]
  clusters: GraphCluster[]
  positions: Record<string, [number, number]> | null
  layoutVersion: number
  layoutPending?: boolean
}

export type NodeDetail = GraphNode & {
  provenanceDetail: {
    chatId: string | null
    messageIndex: number | null
    snippet: string
  }
  incoming: GraphChain[]
  outgoing: GraphChain[]
  related: { id: string; text: string; kind: NodeKind }[]
  cards: { id: string; front: string; state: CardState }[]
}

export type DeletePreview = {
  deleted: number
  orphanedChains: number
  blastRadius: { id: string; text: string; reason: string }[]
  previewToken: string
}

export type ChainTableRow = {
  groupId: string
  groupLabel: string
  chainId: string
  from: string
  to: string
  kind: ChainKind
}

// ───────────────────────────── Chats ─────────────────────────────
// Mirrors ChatPageDto / ClassroomViewDto / TurnBody in openapi/openapi.json (the generated schema.d.ts
// cannot be regenerated yet: the spec has duplicate operationIds, a server-side fix).

export type ChatKind = "teach" | "practice"
export type ChatPhase = "probe" | "plan" | "teach" | "practice" | "done"
export type PendingTool =
  "probe" | "recall" | "quiz" | "explain" | "ask_user_question" | "problem"
export type ProblemMode = "recall" | "discrimination" | "composition"

export type ChatSummary = {
  id: string
  kind: ChatKind
  title: string
  phase: ChatPhase
  updatedAt: number
  messageCount: number
  pending: boolean
  jobs: number
}

export type ChatPage = {
  items: ChatSummary[]
  nextCursor?: string | null
  total: number
}

export type ClassroomMessage = {
  id: string
  role: "user" | "assistant" | "tool" | "steer" | "compaction"
  text: string
  thinking?: string | null
  interrupted?: boolean | null
  calls: { id: string; name: string }[]
  /** For an answer: the question call it answers. */
  answersCallId?: string | null
  createdAt: number | null
}

export type PendingQuestion = {
  callId: string
  tool: PendingTool
  prompt: string
  options?: { key: string; text: string }[] | null
  allowFreeText: boolean
  multiSelect: boolean
  confidenceScale?: number | null
  /** The answer value that means "I don't know" (options only). */
  idkKey?: string | null
  problem?: {
    mode: ProblemMode
    statement: string
    rungs?: string[] | null
  } | null
}

export type PlanNodeStatus = "current" | "visited" | "upcoming"

export type JobStatus = "running" | "done" | "failed"

/** A job as embedded in `ClassroomView`. */
export type ClassroomJob = {
  id: string
  kind: string
  status: JobStatus
  title: string
  artifactId?: string | null
}

/** A job in the workspace list (`GET …/jobs`). */
export type JobItem = ClassroomJob & {
  chatId: string
  createdAt: number
  error?: string | null
}

export type TurnErrorInfo = {
  code: string
  message: string
  retryable: boolean
  retryAfterMs?: number | null
  /** The server stopped mid-turn; offer Resume. */
  interrupted: boolean
}

export type ProblemResult = {
  callId: string
  outcome: string
  verdict: string
  rung?: string | null
  missClass?: string | null
  retest: boolean
}

export type ClassroomView = {
  id: string
  kind: ChatKind
  title: string
  phase: ChatPhase
  viewVersion: number
  messages: ClassroomMessage[]
  pending?: PendingQuestion | null
  plan?: {
    nodes: {
      id: string
      title: string
      summary: string
      status: PlanNodeStatus
    }[]
    edges: { from: string; to: string }[]
    currentId?: string | null
  } | null
  probeProgress: { asked: number; max: number }
  verdicts: { callId: string; correct: boolean; explanation?: string | null }[]
  problemResult?: ProblemResult | null
  jobs: ClassroomJob[]
  turn: {
    state: "idle" | "running" | "queued" | "error"
    turnId?: string | null
    startedAt?: number | null
    /** The request that started the turn; post it again to retry after an error. */
    clientId?: string | null
    /** While `queued` on a rate limit: when the next attempt starts (epoch ms). */
    retryAt?: number | null
    error?: TurnErrorInfo | null
  }
}

/** The learner's answer to the pending question. */
export type AnswerInput = {
  /** Picked option keys (or the question's `idkKey`); several only when `multiSelect`. */
  choice?: string[]
  text?: string
  readingNotes?: string
  confidence?: number
  /** Problems only: ask for the next hint rung. */
  hint?: boolean
}

export type TurnBody =
  | { type: "user"; text: string; clientId: string }
  | { type: "answer"; callId: string; answer: AnswerInput; clientId: string }
  | { type: "steer"; text: string; clientId: string }
  | { type: "resume"; clientId: string }

export type PdfUpload = {
  jobId: string
  artifactId: string
  chatId: string
  bytes: number
}

export type TurnAcceptance =
  "started" | "duplicate" | "steerQueued" | "recorded"

export type CreateChatBody = {
  kind: ChatKind
  topic?: string
  seedNodeIds?: string[]
}

// ───────────────────────────── Review ─────────────────────────────

export type CardState = "new" | "learning" | "review" | "relearning"
export type Rating = 1 | 2 | 3 | 4

export type ReviewCard = {
  id: string
  front: string
  options?: { key: string; text: string }[]
  kind: string
  sourceDecisionId?: string
  due: number
  state: CardState
}

export type ReviewQueue = { items: ReviewCard[]; nextCursor: string | null }

/** Time until the next review for each rating, in days (fractions for learning steps). */
export type IntervalPreview = {
  again: number
  hard: number
  good: number
  easy: number
}

export type AnswerResult = {
  correct: boolean
  correctKey: string
  explanation: string
  rating: Rating
  next: { due: number; intervalDays: number }
  preview: IntervalPreview
}

export type ReviewStats = {
  dueNow: number
  dueToday: number
  newToday: number
  streak: number
  reviewedToday: number
  retention30d: number
  forecast: number[]
}

export type CardListItem = ReviewCard & {
  stability: number
  difficulty: number
  reps: number
  lapses: number
}

export type ReviewLog = { ts: number; rating: Rating; deviceId: string }

/** One card for the browser drawer: the answer, the rationale and the review history. */
export type CardDetail = CardListItem & {
  correctKey: string
  rationale: string
  history: ReviewLog[]
}

export type AnswerBody = {
  choiceKey: string
  clientId: string
  confidence?: number
}

export type GradeBody = { rating: Rating; clientId: string }

// ───────────────────────────── Digest ─────────────────────────────

export type DecisionBasis = "stated" | "inferred"

export type DigestDecision = {
  id: string
  title: string
  basis: DecisionBasis
  alternatives: { option: string; rejectedBecause: string }[]
  chosen: string
  cardIds: string[]
}

export type DigestSession = {
  sessionId: string
  source: NodeSource
  project: string
  summary: string
  startedAt: number
  decisions: DigestDecision[]
}

export type DigestDay = { day: string; sessions: DigestSession[] }

/** Result of regenerating a decision's cards; `rejected` lists drafts that failed validation. */
export type RegenerateResult = {
  cardIds: string[]
  saved?: number
  rejected?: { front: string; reason: string }[]
}

export type DecisionDetail = DigestDecision & {
  why: string
  risk: string
  files: string[]
  commits: { sha: string; subject: string; at: number }[]
}

// ───────────────────────────── Ingest ─────────────────────────────

export type IngestCandidate = {
  path: string
  sessions: number
  lastSeen: number
  source: NodeSource
}

export type IngestRun = {
  id: string
  startedAt: number
  finishedAt?: number | null
  report: IngestReport
}

export type IngestSourcesBody = {
  claude: boolean
  codex: boolean
  agy: boolean
  git: boolean
  /** Project directories whose sessions belong to this workspace; empty disables ingest. */
  allow: string[]
  sinceDays: number
}

/** `IngestSources` on the wire. */
export type IngestSourcesWire = {
  sources: IngestSourcesBody
  enabled: boolean
  runningId?: string | null
  candidates: IngestCandidate[]
  warnings: { path: string; otherPath: string; workspaceId: string }[]
}

/**
 * What the screens read: the wire `IngestSources` flattened (`sources.*` next to `candidates`) with the
 * overlap warnings renamed for display.
 */
export type IngestSources = IngestSourcesBody & {
  enabled: boolean
  runningId: string | null
  candidates: IngestCandidate[]
  overlaps: { path: string; workspaceId: string; otherPath: string }[]
}

// ───────────────────────────── Path map (openapi-typescript shape) ─────────────────────────────

type Json<T> = { content: { "application/json": T } }
type Res<T> = {
  200: { headers: Record<string, unknown>; content: { "application/json": T } }
}
type Accepted<T> = {
  202: { headers: Record<string, unknown>; content: { "application/json": T } }
}
type Done = { 204: { headers: Record<string, unknown>; content?: never } }
type PathParam<P> = [P] extends [Record<string, never>]
  ? { path?: never }
  : { path: P }
type Params<P, Q> = { query?: Q; header?: never; cookie?: never } & PathParam<P>
type WsP = { ws: string }
type Unused = {
  parameters: { query?: never; header?: never; path?: never; cookie?: never }
}

type GetOp<R, P = Record<string, never>, Q = never> = {
  parameters: Params<P, Q>
  requestBody?: never
  responses: Res<R>
}
type SendOp<R, B, P = Record<string, never>, Q = never> = {
  parameters: Params<P, Q>
  requestBody: Json<B>
  responses: Res<R>
}
type BodylessOp<R, P = Record<string, never>> = {
  parameters: Params<P, never>
  requestBody?: never
  responses: Res<R>
}
type DeleteOp<P = Record<string, never>> = {
  parameters: Params<P, never>
  requestBody?: never
  responses: Done
}

type MultipartOp<R, P = Record<string, never>, Q = never> = {
  parameters: Params<P, Q>
  requestBody: { content: { "multipart/form-data": { file: string } } }
  responses: Res<R>
}

type Method<K extends string, V> = { [P in K]: V }
type NoMethods = {
  get?: never
  put?: never
  post?: never
  delete?: never
  patch?: never
  options?: never
  head?: never
  trace?: never
}
type Path<M extends object> = Unused & Omit<NoMethods, keyof M> & M

type WsId = WsP & { id: string }

export interface PendingPaths {
  "/search": Path<
    Method<
      "get",
      GetOp<SearchHit[], Record<string, never>, { q: string; limit?: number }>
    >
  >
  "/accounts": Path<Method<"get", GetOp<{ items: AccountDetail[] }>>>
  "/accounts/rescan": Path<
    Method<"post", BodylessOp<{ items: AccountDetail[] }>>
  >
  "/accounts/claude": Path<
    Method<"post", SendOp<AccountDetail, { path: string }>>
  >
  "/accounts/{id}": Path<
    Method<
      "patch",
      SendOp<AccountDetail, { disabled: boolean }, { id: string }>
    >
  >
  "/settings": Path<
    Method<"get", GetOp<SettingsDoc>> &
      Method<"put", SendOp<SettingsDoc, SettingsDoc>>
  >
  "/settings/catalog": Path<Method<"get", GetOp<SettingsCatalog>>>
  "/settings/test": Path<
    Method<"post", SendOp<ProfilePingResult, ProfilePingBody>>
  >
  "/diagnostics": Path<Method<"post", BodylessOp<{ text: string }>>>
  "/trash": Path<Method<"get", GetOp<TrashItem[]>>>
  "/trash/{id}/restore": Path<
    Method<"post", BodylessOp<WorkspaceDetail, { id: string }>>
  >
  "/trash/{id}": Path<Method<"delete", DeleteOp<{ id: string }>>>
  "/workspaces": Path<
    Method<
      "get",
      GetOp<WorkspaceListItem[], Record<string, never>, { archived?: boolean }>
    > &
      Method<"post", SendOp<WorkspaceDetail, CreateWorkspaceBody>>
  >
  "/workspaces/import": Path<
    Method<"post", MultipartOp<{ workspaceId: string }>>
  >
  "/workspaces/{ws}": Path<
    Method<"get", GetOp<WorkspaceDetail, WsP>> &
      Method<"patch", SendOp<WorkspaceDetail, PatchWorkspaceBody, WsP>> &
      Method<
        "delete",
        {
          parameters: Params<WsP, never>
          requestBody: Json<{ confirmName: string }>
          responses: Done
        }
      >
  >
  "/workspaces/{ws}/archive": Path<
    Method<"post", BodylessOp<WorkspaceDetail, WsP>>
  >
  "/workspaces/{ws}/restore": Path<
    Method<"post", BodylessOp<WorkspaceDetail, WsP>>
  >
  "/workspaces/{ws}/repair": Path<
    Method<"post", BodylessOp<WorkspaceDetail, WsP>>
  >
  "/workspaces/{ws}/export": Path<
    Method<"post", BodylessOp<{ jobId: string }, WsP>>
  >
  "/workspaces/{ws}/graph": Path<Method<"get", GetOp<GraphSnapshot, WsP>>>
  "/workspaces/{ws}/graph/nodes/{id}": Path<
    Method<"get", GetOp<NodeDetail, WsId>>
  >
  "/workspaces/{ws}/graph/search": Path<
    Method<
      "get",
      GetOp<
        { id: string; text: string; kind: NodeKind; score: number }[],
        WsP,
        { q: string }
      >
    >
  >
  "/workspaces/{ws}/graph/relayout": Path<
    Method<"post", BodylessOp<{ layoutVersion: number }, WsP>>
  >
  "/workspaces/{ws}/graph/delete-preview": Path<
    Method<"post", SendOp<DeletePreview, { nodeIds: string[] }, WsP>>
  >
  "/workspaces/{ws}/graph/nodes": Path<
    Method<
      "delete",
      SendOp<
        { deleted: number },
        { nodeIds: string[]; previewToken: string },
        WsP
      >
    >
  >
  "/workspaces/{ws}/graph/chains": Path<
    Method<"get", GetOp<ChainTableRow[], WsP, { selection?: string }>>
  >
  "/workspaces/{ws}/graph/practice": Path<
    Method<"post", SendOp<{ chatId: string }, { nodeIds: string[] }, WsP>>
  >
  "/workspaces/{ws}/chats": Path<
    Method<"get", GetOp<ChatPage, WsP>> &
      Method<
        "post",
        {
          parameters: Params<WsP, never>
          requestBody: Json<CreateChatBody>
          responses: Accepted<{ chatId: string; turnId: string }>
        }
      >
  >
  "/workspaces/{ws}/chats/{id}": Path<
    Method<"get", GetOp<ClassroomView, WsId>> &
      Method<"patch", SendOp<ChatSummary, { title: string }, WsId>> &
      Method<"delete", DeleteOp<WsId>>
  >
  "/workspaces/{ws}/chats/{id}/turn": Path<
    Method<
      "post",
      {
        parameters: Params<WsId, never>
        requestBody: Json<TurnBody>
        responses: Accepted<{ turnId: string; status: TurnAcceptance }>
      }
    >
  >
  "/workspaces/{ws}/chats/{id}/cancel": Path<
    Method<
      "post",
      BodylessOp<{ cancelled: boolean; turnId?: string | null }, WsId>
    >
  >
  "/workspaces/{ws}/sources/pdf": Path<
    Method<
      "post",
      {
        parameters: Params<WsP, { chat: string }>
        requestBody: { content: { "multipart/form-data": { file: string } } }
        responses: Accepted<PdfUpload>
      }
    >
  >
  "/workspaces/{ws}/jobs": Path<Method<"get", GetOp<{ items: JobItem[] }, WsP>>>
  "/workspaces/{ws}/jobs/{id}/abort": Path<
    Method<"post", BodylessOp<JobItem, WsId>>
  >
  "/workspaces/{ws}/review/queue": Path<
    Method<"get", GetOp<ReviewQueue, WsP, { limit?: number }>>
  >
  "/workspaces/{ws}/review/{id}/answer": Path<
    Method<"post", SendOp<AnswerResult, AnswerBody, WsId>>
  >
  "/workspaces/{ws}/review/{id}/grade": Path<
    Method<"post", SendOp<ReviewCard, GradeBody, WsId>>
  >
  "/workspaces/{ws}/review/preview/{id}": Path<
    Method<"get", GetOp<IntervalPreview, WsId>>
  >
  "/workspaces/{ws}/review/stats": Path<Method<"get", GetOp<ReviewStats, WsP>>>
  "/workspaces/{ws}/review/cards": Path<
    Method<"get", GetOp<CardListItem[], WsP, { state?: CardState; q?: string }>>
  >
  "/workspaces/{ws}/review/cards/{id}": Path<Method<"get", GetOp<CardDetail, WsId>>>
  "/workspaces/{ws}/review/sync": Path<
    Method<"post", BodylessOp<Record<string, never>, WsP>>
  >
  "/workspaces/{ws}/digest": Path<
    Method<
      "get",
      GetOp<DigestDay[], WsP, { from?: string; to?: string; source?: string }>
    >
  >
  "/workspaces/{ws}/digest/decisions/{id}": Path<
    Method<"get", GetOp<DecisionDetail, WsId>>
  >
  "/workspaces/{ws}/digest/decisions/{id}/cards": Path<
    Method<"post", BodylessOp<RegenerateResult, WsId>>
  >
  "/workspaces/{ws}/ingest/sources": Path<
    Method<"get", GetOp<IngestSourcesWire, WsP>> &
      Method<"put", SendOp<IngestSourcesWire, IngestSourcesBody, WsP>>
  >
  "/workspaces/{ws}/ingest/run": Path<
    Method<
      "post",
      {
        parameters: Params<WsP, never>
        requestBody: Json<{ sinceDays?: number | null }>
        responses: Accepted<{ runId: string }>
      }
    >
  >
  "/workspaces/{ws}/ingest/runs": Path<
    Method<"get", GetOp<{ items: IngestRun[] }, WsP>>
  >
  "/workspaces/{ws}/ingest/runs/{id}/cancel": Path<
    Method<
      "post",
      {
        parameters: Params<WsId, never>
        requestBody?: never
        responses: Done
      }
    >
  >
}
