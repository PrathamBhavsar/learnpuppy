// Everything the renderer reads for one frame. The engine owns and mutates this; the draw modules
// only read it, so a frame is a pure function of FrameState + time.
import type { Scene } from "../model/scene"

import type { Cam, Size } from "./camera"
import type { CanvasTheme } from "./theme"

export type LabelMode = "auto" | "always" | "never"

/** A one-shot animation on a node, measured from `t0` (performance.now()). */
export type NodeAnim = { idx: number; t0: number }

export const ANIM_MS = {
  birth: 420,
  glow: 600,
  pulse: 420,
  dying: 200,
} as const

export type FrameState = {
  size: Size
  dpr: number
  cam: Cam
  /** k / fitK: the zoom relative to the fit view; level-of-detail keys off this. */
  z: number
  theme: CanvasTheme
  scene: Scene
  /** Live node positions (tweened between layouts). */
  px: Float32Array
  py: Float32Array
  /** Base node side in px at fit zoom. */
  basePx: number
  visible: Uint8Array | null
  match: Uint8Array | null
  matchList: Uint32Array | null
  selected: Uint8Array
  selectedAny: boolean
  activeIdx: number
  hoverIdx: number
  /** 0..1 progress of the hover dim, tweened. */
  hoverMix: number
  /** For the hovered node: 1 marks a neighbour. Null when nothing is hovered. */
  neighbour: Uint8Array | null
  /** 1 = drawn individually (selected, hovered, animating); skipped in the bulk pass. */
  special: Uint8Array
  /** Per-cluster fade-in alpha during load-in; null once settled. Last slot = unclustered. */
  clusterAlpha: Float32Array | null
  edgesAlpha: number
  hullFade: number
  births: NodeAnim[]
  dying: NodeAnim[]
  pulses: NodeAnim[]
  deleteTint: Uint8Array | null
  now: number
  hulls: boolean
  labels: LabelMode
  performance: boolean
  /** Pan/zoom in progress: draw cheaper (straight edges, no labels). */
  interacting: boolean
}

/** Alpha of the dimmed layers (docs/web/05 #17: non-neighbours dim to 25%). */
export const HOVER_DIM = 0.25
export const SEARCH_DIM = 0.16
