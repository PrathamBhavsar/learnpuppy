import type {
  ChainKind,
  GraphChain,
  GraphCluster,
  GraphNode,
  GraphSnapshot,
  NodeDetail,
  NodeKind,
  NodeSource,
} from "@/api/pending-types"

import { DAY, mulberry32, pick } from "./rng"

type Topic = { label: string; claims: string[] }

const TOPICS: Topic[] = [
  {
    label: "Futures and polling",
    claims: [
      "A future does nothing until it is polled",
      "poll returns Ready or Pending and registers a waker",
      "An async fn compiles to a state machine enum",
      "Dropping a future cancels it at the next await point",
      "Futures are lazy; calling an async fn only builds the state machine",
      "A Pending poll must arrange a wake-up or the task stalls",
      "select! cancels the losing branches by dropping them",
      "Poll is cooperative: a long computation blocks the executor thread",
      "join! polls all branches on the same task",
    ],
  },
  {
    label: "Pinning",
    claims: [
      "Pin prevents a value from moving in memory",
      "Self-referential futures need stable addresses",
      "Unpin types may be moved even when pinned",
      "Box::pin moves a future to the heap so it can be pinned",
      "pin! pins a future on the stack for the current scope",
      "Pin<&mut T> exposes get_mut only for Unpin types",
      "Moving a pinned future after its first poll is undefined behaviour",
      "Structural pinning projects pins to fields",
    ],
  },
  {
    label: "Executors and wakers",
    claims: [
      "A waker re-schedules its task on the executor queue",
      "Wakers are reference counted and cheap to clone",
      "A work-stealing scheduler balances tasks across threads",
      "Each worker thread owns a local run queue",
      "spawn requires the future to be Send on a multi-threaded runtime",
      "block_on parks the current thread until the future completes",
      "LocalSet runs !Send futures on one thread",
      "Task budgets force a yield after a number of polls",
    ],
  },
  {
    label: "Tokio I/O driver",
    claims: [
      "mio wraps epoll and kqueue behind one readiness API",
      "Readiness events wake the task that registered interest",
      "AsyncRead and AsyncWrite are poll-based traits",
      "Edge-triggered readiness requires reading until WouldBlock",
      "The reactor runs on its own thread or inside the worker park",
      "Timers live in a hierarchical timing wheel",
      "spawn_blocking moves blocking work to a dedicated thread pool",
      "File I/O is blocking and goes through the blocking pool",
    ],
  },
  {
    label: "Channels and sync",
    claims: [
      "mpsc channels apply backpressure through bounded capacity",
      "oneshot channels deliver exactly one value",
      "broadcast channels clone each message for every receiver",
      "A Mutex guard held across await can deadlock the runtime",
      "tokio::sync::Mutex is async-aware but slower than std for short sections",
      "Semaphore permits bound concurrency",
      "watch channels keep only the latest value",
      "Notify wakes one or all waiting tasks without carrying data",
    ],
  },
  {
    label: "Cancellation and shutdown",
    claims: [
      "Cancellation safety decides whether a future may be dropped mid-await",
      "tokio::select! requires cancel-safe branches to avoid losing data",
      "CancellationToken propagates shutdown through a task tree",
      "Graceful shutdown drains in-flight requests before closing",
      "JoinHandle::abort stops a task at its next yield",
      "Dropping the runtime cancels all spawned tasks",
      "Structured concurrency ties child task lifetime to a scope",
      "timeout wraps a future and drops it when the deadline passes",
    ],
  },
]

const SOURCES: NodeSource[] = ["claude", "codex", "agy", "git", "chat"]
const KINDS: NodeKind[] = ["claim", "concept", "procedure", "decision"]
const CHAIN_KINDS: ChainKind[] = ["supports", "implies", "contrasts", "part_of"]

export type GraphFixture = {
  snapshot: GraphSnapshot
  details: Map<string, NodeDetail>
}

/** Builds `count` nodes across the topics with deterministic layout, chains and degrees. */
export function buildGraph(
  wsId: string,
  count: number,
  now: number,
  seed: number
): GraphFixture {
  const rand = mulberry32(seed)
  const sessions = Array.from({ length: 12 }, (_, i) => `sess_${wsId}_${i + 1}`)
  const nodes: GraphNode[] = []
  const clusters: GraphCluster[] = TOPICS.map((t, i) => ({
    id: `cl_${wsId}_${i + 1}`,
    label: t.label,
    nodeIds: [],
  }))
  const positions: Record<string, [number, number]> = {}

  for (let i = 0; i < count; i++) {
    const topicIndex = i % TOPICS.length
    const topic = TOPICS[topicIndex] as Topic
    const base =
      topic.claims[Math.floor(i / TOPICS.length) % topic.claims.length]
    const variant = Math.floor(i / (TOPICS.length * topic.claims.length))
    const id = `n_${wsId.slice(3)}_${String(i + 1).padStart(3, "0")}`
    const angle = (topicIndex / TOPICS.length) * Math.PI * 2
    const cx = Math.cos(angle) * 520
    const cy = Math.sin(angle) * 520
    positions[id] = [
      Math.round(cx + (rand() - 0.5) * 520),
      Math.round(cy + (rand() - 0.5) * 520),
    ]
    nodes.push({
      id,
      text: variant === 0 ? (base as string) : `${base} (case ${variant + 1})`,
      kind: pick(rand, KINDS),
      sessionId: pick(rand, sessions),
      provenance: pick(rand, SOURCES),
      createdAt: now - Math.floor(rand() * 40 * DAY),
      degree: 0,
    })
    clusters[topicIndex]?.nodeIds.push(id)
  }

  const chains: GraphChain[] = []
  const seen = new Set<string>()
  const add = (from: string, to: string) => {
    const key = `${from}>${to}`
    if (from === to || seen.has(key) || seen.has(`${to}>${from}`)) return
    seen.add(key)
    chains.push({
      id: `ch_${wsId.slice(3)}_${String(chains.length + 1).padStart(3, "0")}`,
      from,
      to,
      kind: pick(rand, CHAIN_KINDS),
    })
  }
  for (const cluster of clusters) {
    cluster.nodeIds.forEach((id, i) => {
      if (i > 0) add(cluster.nodeIds[i - 1] as string, id)
      if (i > 2 && rand() < 0.45)
        add(cluster.nodeIds[Math.floor(rand() * i)] as string, id)
    })
  }
  const target = Math.round(count * 1.3)
  for (let guard = 0; chains.length < target && guard < count * 20; guard++) {
    add(
      (nodes[Math.floor(rand() * count)] as GraphNode).id,
      (nodes[Math.floor(rand() * count)] as GraphNode).id
    )
  }

  const byId = new Map(nodes.map((n) => [n.id, n]))
  for (const chain of chains) {
    ;(byId.get(chain.from) as GraphNode).degree += 1
    ;(byId.get(chain.to) as GraphNode).degree += 1
  }

  const details = new Map<string, NodeDetail>()
  for (const node of nodes) {
    const incoming = chains.filter((c) => c.to === node.id)
    const outgoing = chains.filter((c) => c.from === node.id)
    const relatedIds = [
      ...incoming.map((c) => c.from),
      ...outgoing.map((c) => c.to),
    ]
    details.set(node.id, {
      ...node,
      provenanceDetail: {
        chatId: node.provenance === "chat" ? "c_teach_quiz" : null,
        messageIndex: node.provenance === "chat" ? 4 : null,
        snippet: `…so ${node.text.charAt(0).toLowerCase()}${node.text.slice(1)}, which is why the executor treats it that way…`,
      },
      incoming,
      outgoing,
      related: relatedIds.slice(0, 6).map((rid) => {
        const r = byId.get(rid) as GraphNode
        return { id: r.id, text: r.text, kind: r.kind }
      }),
      cards:
        node.degree > 3
          ? [
              {
                id: `card_${node.id}`,
                front: node.text,
                state: "review" as const,
              },
            ]
          : [],
    })
  }

  return {
    snapshot: {
      version: 1,
      nodes,
      chains,
      clusters,
      positions,
      layoutVersion: 1,
    },
    details,
  }
}
