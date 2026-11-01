// Graph screen (docs/web/06): canvas + toolbar + drawer + chain table + selection bar, with the
// loading, empty, error, layout-pending, filtered-out and performance states.
import { AnimatePresence, motion } from "motion/react"
import { useCallback, useEffect, useMemo, useState } from "react"

import { useQueryClient } from "@tanstack/react-query"

import { keys } from "@/api/keys"
import { useRelayout } from "@/api/queries/graph"
import { ContextMenu, ContextMenuTrigger } from "@/components/ui/context-menu"
import { useEndSide } from "@/hooks/use-end-side"
import { useGoto } from "@/hooks/use-goto"
import { usePageTitle } from "@/hooks/use-page-title"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { useWorkspaceName } from "@/hooks/use-workspace"
import { dur, ease } from "@/lib/motion"
import { toastError, toastSuccess } from "@/lib/toast"
import { cn } from "@/lib/utils"
import { useGraphView } from "@/stores/graphView"

import { lodFor } from "./canvas/camera"
import type { GraphEngine, Insets } from "./canvas/engine"
import { GraphCanvas } from "./canvas/GraphCanvas"
import { useGraphCommands } from "./contrib"
import { copy } from "./copy"
import { installGraphTestHook, loadMockGraph } from "./test-hook"
import { useGraphActions } from "./use-graph-actions"
import { useGraphKeys } from "./use-graph-keys"
import { useGraphModel } from "./use-graph-model"
import { useGraphUrl } from "./use-graph-url"
import { ChainTable } from "./ui/ChainTable"
import { CanvasSurface } from "./ui/CanvasSurface"
import { Controls } from "./ui/Controls"
import { DeleteDialog } from "./ui/DeleteDialog"
import { GraphContextMenu } from "./ui/GraphContextMenu"
import { GraphList } from "./ui/GraphList"
import {
  GraphEmpty,
  GraphError,
  GraphLoading,
  LayoutPending,
  NoVisible,
  PerformanceChip,
} from "./ui/GraphStates"
import { Legend } from "./ui/Legend"
import { MarqueeBox } from "./ui/MarqueeBox"
import { Minimap } from "./ui/Minimap"
import { DRAWER_WIDTH, NodeDrawer } from "./ui/NodeDrawer"
import { NodeTag } from "./ui/NodeTag"
import { SelectionBar } from "./ui/SelectionBar"
import { Toolbar } from "./ui/Toolbar"

const TOOLBAR_INSET = 56
/** Px the legend rises while the selection bar is showing: bar height plus its offset plus a gap. */
const LEGEND_LIFT = 64

export default function GraphScreen({ ws }: { ws: string }) {
  usePageTitle(copy.title, useWorkspaceName(ws))
  const goto = useGoto()
  const queryClient = useQueryClient()
  const reduced = useReducedMotion()
  const edge = useEndSide()
  const url = useGraphUrl()
  const model = useGraphModel(ws, url)
  const { scene } = model
  const view = useGraphView()
  const relayout = useRelayout(ws)
  const [engine, setEngine] = useState<GraphEngine | null>(null)
  const [menuNode, setMenuNode] = useState<number>(-1)

  useEffect(() => view.bind(ws), [view.bind, ws]) // eslint-disable-line react-hooks/exhaustive-deps

  const insetsFor = useCallback(
    (drawer: boolean): Insets => ({
      left: edge === "left" && drawer ? DRAWER_WIDTH : 0,
      right: edge === "right" && drawer ? DRAWER_WIDTH : 0,
      top: TOOLBAR_INSET,
      bottom: 8,
    }),
    [edge]
  )

  const actions = useGraphActions({ ws, scene, engine, url, insetsFor })
  const { selection, drawerOpen } = actions
  const selectedIds = useMemo(
    () => new Set(model.selectedIdx),
    [model.selectedIdx]
  )

  const canvasSelection = useMemo(
    () => ({
      indices: model.selectedIdx,
      active: model.activeIdx,
      pulse: actions.pulse,
    }),
    [model.selectedIdx, model.activeIdx, actions.pulse]
  )
  const deleteTint = useMemo(
    () =>
      scene && actions.deleteIds
        ? actions.deleteIds
            .map((id) => scene.index.get(id))
            .filter((i): i is number => i !== undefined)
        : null,
    [scene, actions.deleteIds]
  )
  const engineOptions = useMemo(
    () => ({
      hulls: view.hulls,
      labels: view.labels,
      performance: model.performance,
      followNew: view.followNew,
    }),
    [view.hulls, view.labels, model.performance, view.followNew]
  )

  useEffect(() => {
    engine?.setInsets(insetsFor(drawerOpen))
  }, [engine, insetsFor, drawerOpen])

  // Layout still computing: show soft cluster silhouettes instead of nodes.
  const clusterSizes = useMemo(
    () => model.snapshot?.clusters.map((c) => c.nodeIds.length) ?? [],
    [model.snapshot]
  )
  useEffect(() => {
    engine?.setPending(model.layoutPending ? clusterSizes : null)
  }, [engine, model.layoutPending, clusterSizes])

  const onKeyDown = useGraphKeys({
    scene,
    engine,
    visible: model.visible,
    activeIdx: model.activeIdx,
    selectedCount: selection.selected.length,
    pick: actions.pickIdx,
    clear: actions.clear,
    openDrawer: actions.openDrawer,
    requestDelete: () => actions.requestDelete(selection.selected),
  })

  useGraphCommands({
    fit: () => engine?.fit(),
    toggleHulls: () => view.setPref("hulls", !view.hulls),
    toggleMinimap: () => view.setPref("minimap", !view.minimap),
    toggleTable: () => view.setPref("table", !view.table),
    focusSearch: () =>
      document.querySelector<HTMLElement>("[data-screen-search]")?.focus(),
    clearSelection: actions.clear,
    selectionCount: selection.selected.length,
  })

  useEffect(() => {
    if (!scene || !engine) return
    return installGraphTestHook({
      nodeCount: () => scene.count,
      selection: () => [...useGraphView.getState().selection.selected],
      active: () => useGraphView.getState().selection.active,
      hover: () => scene.ids[engine.hovered] ?? null,
      camera: () => ({ ...engine.camera.cam, z: engine.zoomLevel }),
      screenPosition: (id) => {
        const idx = scene.index.get(id)
        const pos = idx === undefined ? null : engine.nodeScreen(idx)
        const rect = document
          .querySelector("[data-testid='graph-canvas']")
          ?.getBoundingClientRect()
        return pos && rect
          ? { x: rect.left + pos[0], y: rect.top + pos[1] }
          : null
      },
      frameStats: () => engine.stats(),
      resetFrameStats: () => engine.resetStats(),
      hullAlpha: () => lodFor(engine.zoomLevel).hulls,
      loadFixture: async (n) => {
        await loadMockGraph(ws, n)
        await queryClient.invalidateQueries({ queryKey: keys.ws(ws).graph() })
      },
    })
  }, [scene, engine, ws, queryClient])

  const handlers = useMemo(
    () => ({
      click: ({
        idx,
        shift,
        meta,
      }: {
        idx: number
        shift: boolean
        meta: boolean
      }) => {
        if (idx >= 0) actions.pickIdx(idx, shift || meta ? "toggle" : "replace")
        else if (!shift) actions.clear()
      },
      dblclick: ({ idx }: { idx: number }) =>
        actions.pickIdx(idx, "replace", { fly: "group", open: true }),
      marquee: ({ idx }: { idx: number[] }) => actions.marquee(idx),
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [actions.pickIdx, actions.clear, actions.marquee]
  )

  const onContextMenu = (e: React.MouseEvent<HTMLElement>) => {
    const rect = e.currentTarget.getBoundingClientRect()
    const idx = engine?.hitAt(e.clientX - rect.left, e.clientY - rect.top) ?? -1
    if (idx < 0 || !scene) {
      e.preventDefault()
      return
    }
    setMenuNode(idx)
    if (!selectedIds.has(idx)) actions.pickIdx(idx, "replace", { fly: "none" })
  }

  const menuId = menuNode >= 0 ? (scene?.ids[menuNode] ?? null) : null
  const menuIds =
    menuId && selection.selected.includes(menuId)
      ? [...selection.selected]
      : menuId
        ? [menuId]
        : []

  const nodes = model.snapshot?.nodes.length ?? 0
  const body = (() => {
    if (model.query.isPending) return <GraphLoading />
    if (model.query.isError && !model.snapshot)
      return (
        <GraphError
          error={model.query.error}
          onRetry={() => model.query.refetch()}
        />
      )
    if (model.snapshot && nodes === 0)
      return (
        <GraphEmpty
          onLearn={() => goto(`/w/${ws}/learn`)}
          onIngest={() => goto(`/w/${ws}/ingest`)}
        />
      )
    return null
  })()
  const populated = body === null && model.snapshot !== undefined
  const activeNode = model.activeIdx
  const drawerVisible = drawerOpen && activeNode >= 0
  const announcement =
    scene && activeNode >= 0 && selection.active
      ? copy.list.announceSelected(
          scene.texts[activeNode] as string,
          scene.degree[activeNode] as number
        )
      : selection.selected.length === 0
        ? copy.list.announceCleared
        : copy.selection.count(selection.selected.length)

  return (
    <section
      data-testid="screen-graph"
      aria-busy={model.query.isPending}
      className="flex h-full min-h-0 flex-col"
    >
      <h1 tabIndex={-1} data-page-title className="sr-only">
        {copy.title}
      </h1>
      <div
        data-testid="graph-stage"
        className="relative min-h-0 flex-1 overflow-hidden bg-background"
      >
        {body}
        {populated ? (
          <>
            <ContextMenu onOpenChange={(open) => !open && setMenuNode(-1)}>
              <ContextMenuTrigger asChild>
                <CanvasSurface
                  aria-label={copy.canvasLabel(model.totalNodes)}
                  aria-describedby="graph-help"
                  data-testid="graph-focus"
                  onKeyDown={onKeyDown}
                  onContextMenu={onContextMenu}
                  className="absolute inset-0 outline-none focus-visible:ring-1 focus-visible:ring-ring/50 focus-visible:ring-inset"
                >
                  <GraphCanvas
                    scene={scene}
                    visible={model.visible}
                    match={model.match}
                    selection={canvasSelection}
                    options={engineOptions}
                    insets={insetsFor(drawerOpen)}
                    deleteTint={deleteTint}
                    handlers={handlers}
                    onEngine={setEngine}
                  />
                </CanvasSurface>
              </ContextMenuTrigger>
              <GraphContextMenu
                ws={ws}
                nodeId={menuId}
                text={
                  menuNode >= 0 && scene
                    ? (scene.texts[menuNode] as string)
                    : ""
                }
                count={menuIds.length}
                onPractice={() => actions.startPractice(menuIds)}
                onDelete={() => actions.requestDelete(menuIds)}
                onOpenChat={(chat) => goto(`/w/${ws}/learn/${chat}`)}
              />
            </ContextMenu>
            <p id="graph-help" className="sr-only">
              {copy.canvasHelp}
            </p>

            {scene ? <NodeTag engine={engine} scene={scene} /> : null}
            <MarqueeBox engine={engine} />

            {scene ? (
              <Toolbar
                query={url.q}
                onQuery={url.setQuery}
                matchCount={model.matchCount}
                searching={model.searching}
                onSubmitSearch={() => {
                  const first = model.match ? model.match.indexOf(1) : -1
                  if (first >= 0)
                    actions.pickIdx(first, "replace", { fly: "node" })
                }}
                sources={url.sources}
                onSources={url.setSources}
                clusters={scene.clusters}
                cluster={url.cluster}
                onCluster={url.setCluster}
                minDegree={view.minDegree}
                maxDegree={model.maxDegree}
                onMinDegree={view.setMinDegree}
                filtersActive={model.filtersActive}
                onClearFilters={() => {
                  url.clearFilters()
                  view.setMinDegree(0)
                }}
                hulls={view.hulls}
                onHulls={(v) => view.setPref("hulls", v)}
                labels={view.labels}
                onLabels={(v) => view.setPref("labels", v)}
                minimap={view.minimap}
                onMinimap={(v) => view.setPref("minimap", v)}
                followNew={view.followNew}
                onFollowNew={(v) => view.setPref("followNew", v)}
                table={view.table}
                onTable={(v) => view.setPref("table", v)}
                counts={copy.counts(
                  model.counts.nodes,
                  model.counts.chains,
                  model.filtersActive ? model.totalNodes : undefined
                )}
                dockInset={drawerVisible ? DRAWER_WIDTH : 0}
              />
            ) : null}

            {model.layoutPending ? <LayoutPending /> : null}
            {scene && model.filtersActive && model.counts.nodes === 0 ? (
              <NoVisible
                onClear={() => {
                  url.clearFilters()
                  view.setMinDegree(0)
                }}
              />
            ) : null}

            <motion.div
              // Lifts clear of the selection bar. Transform only, so it never animates layout.
              animate={{ y: selection.selected.length > 0 ? -LEGEND_LIFT : 0 }}
              transition={{
                duration: reduced ? 0 : dur.base,
                ease: ease.inout,
              }}
              className="pointer-events-none absolute start-3 bottom-3 z-10 flex flex-col items-start gap-2"
            >
              {model.overLimit ? (
                <PerformanceChip
                  nodes={nodes}
                  on={model.performance}
                  onToggle={() =>
                    view.setPref(
                      "performance",
                      model.performance ? "off" : "on"
                    )
                  }
                />
              ) : null}
              {scene ? <Legend /> : null}
            </motion.div>

            {scene ? (
              <motion.div
                animate={{
                  x: drawerVisible
                    ? edge === "right"
                      ? -DRAWER_WIDTH
                      : DRAWER_WIDTH
                    : 0,
                }}
                transition={{
                  duration: reduced ? 0 : dur.base,
                  ease: ease.inout,
                }}
                className={cn(
                  "pointer-events-none absolute end-3 bottom-3 z-10 flex flex-col items-end gap-2"
                )}
              >
                {view.minimap ? (
                  <Minimap
                    engine={engine}
                    scene={scene}
                    visible={model.visible}
                  />
                ) : null}
                <Controls
                  onZoomIn={() => engine?.zoomBy(1.4)}
                  onZoomOut={() => engine?.zoomBy(1 / 1.4)}
                  onFit={() => engine?.fit()}
                  onRelayout={() =>
                    relayout.mutate(undefined, {
                      onSuccess: () => toastSuccess(copy.relayoutDone),
                      onError: (error) =>
                        toastError(error, copy.relayoutFailed),
                    })
                  }
                  relayoutPending={relayout.isPending}
                />
              </motion.div>
            ) : null}

            <AnimatePresence>
              {drawerVisible && scene ? (
                <NodeDrawer
                  key="drawer"
                  ws={ws}
                  scene={scene}
                  idx={model.activeIdx}
                  edge={edge}
                  focusOnOpen={actions.focusDrawer}
                  onClose={() => {
                    actions.closeDrawer()
                    document
                      .querySelector<HTMLElement>("[data-testid='graph-focus']")
                      ?.focus()
                  }}
                  onHop={(id) => {
                    const i = scene.index.get(id)
                    if (i !== undefined)
                      actions.pickIdx(i, "replace", { fly: "node" })
                  }}
                  onPractice={(id) => actions.startPractice([id])}
                  onDelete={(id) => actions.requestDelete([id])}
                  practicePending={actions.practicePending}
                />
              ) : null}
            </AnimatePresence>

            <AnimatePresence>
              {selection.selected.length > 0 ? (
                <SelectionBar
                  key="bar"
                  count={selection.selected.length}
                  onPractice={() => actions.startPractice(selection.selected)}
                  onDelete={() => actions.requestDelete(selection.selected)}
                  onClear={actions.clear}
                  practicePending={actions.practicePending}
                  dockInset={drawerVisible ? DRAWER_WIDTH : 0}
                />
              ) : null}
            </AnimatePresence>

            {scene ? (
              <GraphList
                scene={scene}
                visible={model.visible}
                match={model.match}
                selected={selectedIds}
                active={model.activeIdx}
                onPick={(i) => actions.pickIdx(i, "replace", { open: false })}
                onOpen={(i) => {
                  actions.pickIdx(i, "replace")
                  actions.openDrawer()
                }}
                announcement={announcement}
              />
            ) : null}
          </>
        ) : null}
      </div>

      {populated && scene && view.table ? (
        <ChainTable
          ws={ws}
          scene={scene}
          selection={selection.selected}
          height={view.tableHeight}
          onResize={(h) => view.setPref("tableHeight", h)}
          onClose={() => view.setPref("table", false)}
          onPickNodes={(ids) => actions.pickIds(ids, { fly: "node" })}
        />
      ) : null}

      <DeleteDialog
        ws={ws}
        nodeIds={actions.deleteIds}
        onAnimateOut={actions.animateOut}
        onDeleted={actions.finishDelete}
        onClose={actions.cancelDelete}
      />
    </section>
  )
}
