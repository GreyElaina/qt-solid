import { createContext, createSignal, useContext, type Accessor } from "solid-js"
import { createRenderer } from "solid-js/universal"

import {
  type QtApp,
  type QtNode,
} from "@qt-solid/core"
import {
  canvasFragmentCreate,
  canvasFragmentRequestRepaint,
  canvasFragmentSetProp,
  canvasFragmentSetF64Prop,
  canvasFragmentSetListener,
} from "@qt-solid/core/native"

import { rendererInspectorStore } from "../devtools/inspector-store.ts"
import { currentQtSolidOwnerMetadata, withQtOwnerFrame } from "../devtools/owner-metadata.ts"
import { isQtSolidSourceMetadata, QT_SOLID_SOURCE_META_PROP } from "../devtools/source-metadata.ts"
import { FRAGMENT_ROOT_ID, FragmentRendererNode, writeFragmentProp } from "./fragment.ts"
import { HANDLED_EVENT_NAMES } from "./canvas/dispatch.ts"
import { cleanupHoverOnRemove } from "./canvas/registry.ts"
import {
  WINDOW_EVENT_EXPORTS,
  wiredEventExports,
  setNativeEventHandler,
  forgetNativeEvents,
} from "./host-events.ts"
import {
  bindMotionNode,
  isMotionNodeHandle,
  MOTION_PROP_KEYS,
  type GestureState,
  type DragController,
} from "../app/motion/motion.ts"
import type { MotionComponentProps } from "../app/motion/types.ts"

const FRAGMENT_LISTENER_LAYOUT = 1

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export type QtFlexDirection = "column" | "row"
export type QtAlignItems = "flex-start" | "center" | "flex-end" | "stretch"
export type QtJustifyContent = "flex-start" | "center" | "flex-end"

// ---------------------------------------------------------------------------
// Canvas scope context — provided by Window/Canvas to children
// ---------------------------------------------------------------------------

export interface CanvasScope {
  readonly hostNode: QtNode
  readonly root: FragmentRendererNode
}

export const CanvasScopeContext = createContext<CanvasScope | null>(null)

// ---------------------------------------------------------------------------
// Native widget node — wraps napi QtNode for windows
// ---------------------------------------------------------------------------

export class NativeWidgetNode {
  readonly qtNode: QtNode

  constructor(qtNode: QtNode) {
    this.qtNode = qtNode
  }

  get id(): number {
    return this.qtNode.id
  }

  destroy(): void {
    this.qtNode.destroy()
  }
}

// ---------------------------------------------------------------------------
// Module-level state — initialized via initRenderer(app)
// ---------------------------------------------------------------------------

let currentApp: QtApp | undefined
let rootNode: NativeWidgetNode | undefined

const canonicalNodes = new Map<number, NativeWidgetNode>()

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

function canonNode(qtNode: QtNode): NativeWidgetNode {
  const existing = canonicalNodes.get(qtNode.id)
  if (existing) return existing
  const node = new NativeWidgetNode(qtNode)
  canonicalNodes.set(qtNode.id, node)
  return node
}

function forgetNativeSubtree(node: NativeWidgetNode): void {
  if (!canonicalNodes.has(node.id)) return
  let child: QtNode | null
  try {
    child = node.qtNode.firstChild
  } catch {
    // Node already destroyed on the native side — just clean up maps.
    canonicalNodes.delete(node.id)
    forgetNativeEvents(node.id)
    return
  }
  while (child) {
    const next = child.nextSibling
    const wrapped = canonicalNodes.get(child.id)
    if (wrapped) forgetNativeSubtree(wrapped)
    child = next
  }
  canonicalNodes.delete(node.id)
  forgetNativeEvents(node.id)
}

// ---------------------------------------------------------------------------
// Renderer initialization
// ---------------------------------------------------------------------------

export function initRenderer(app: QtApp): void {
  currentApp = app
  const qtRoot = app.root
  rootNode = canonNode(qtRoot)
}

export function nativeRoot(): NativeWidgetNode {
  if (!rootNode) {
    throw new Error("Renderer not initialized — call initRenderer(app) first")
  }
  return rootNode
}

export function nativeApp(): QtApp {
  if (!currentApp) {
    throw new Error("Renderer not initialized — call initRenderer(app) first")
  }
  return currentApp
}

// ---------------------------------------------------------------------------
// Native widget operations
// ---------------------------------------------------------------------------

export function createNativeWidget(): NativeWidgetNode {
  if (!currentApp) {
    throw new Error("Renderer not initialized — call initRenderer(app) first")
  }
  const qtNode = currentApp.createWidget()
  const node = new NativeWidgetNode(qtNode)
  canonicalNodes.set(qtNode.id, node)
  return node
}

export function insertNativeWidget(parent: NativeWidgetNode, child: NativeWidgetNode, anchor?: NativeWidgetNode | null): void {
  parent.qtNode.insertChild(child.qtNode, anchor?.qtNode ?? null)
}

export function removeNativeWidget(parent: NativeWidgetNode, child: NativeWidgetNode): void {
  if (!canonicalNodes.has(child.id)) return
  forgetNativeSubtree(child)
  parent.qtNode.removeChild(child.qtNode)
  child.destroy()
}

export function destroyChildWidgets(parent: NativeWidgetNode): void {
  let child = parent.qtNode.firstChild
  while (child) {
    const next = child.nextSibling
    const wrapped = canonicalNodes.get(child.id)
    if (wrapped) forgetNativeSubtree(wrapped)
    child.destroy()
    child = next
  }
}

// ---------------------------------------------------------------------------
// Solid.js universal renderer — fragment only
// ---------------------------------------------------------------------------

function useCanvasScope(): CanvasScope {
  const scope = useContext(CanvasScopeContext)
  if (!scope) {
    throw new Error("Canvas scope required — createElement for fragments must be inside a Window or Canvas")
  }
  return scope
}

const fragmentRenderer = createRenderer<FragmentRendererNode>({
  createElement(type) {
    const scope = useCanvasScope()
    const canvasNodeId = scope.hostNode.id
    const fragmentId = canvasFragmentCreate(canvasNodeId, type)
    const node = new FragmentRendererNode(scope.hostNode, fragmentId, type)

    rendererInspectorStore.addCanvas(canvasNodeId)
    rendererInspectorStore.emit({ type: "node-created", canvasNodeId, fragmentId, kind: type })

    const owner = currentQtSolidOwnerMetadata()
    if (owner) {
      rendererInspectorStore.setOwner(canvasNodeId, fragmentId, owner)
    }

    return node
  },

  createTextNode(value) {
    const scope = useCanvasScope()
    const canvasNodeId = scope.hostNode.id
    const fragmentId = canvasFragmentCreate(canvasNodeId, "Text")
    const node = new FragmentRendererNode(scope.hostNode, fragmentId, "Text")

    rendererInspectorStore.addCanvas(canvasNodeId)
    rendererInspectorStore.emit({ type: "node-created", canvasNodeId, fragmentId, kind: "#text" })
    writeFragmentProp(canvasNodeId, fragmentId, "text", String(value))

    const owner = currentQtSolidOwnerMetadata()
    if (owner) {
      rendererInspectorStore.setOwner(canvasNodeId, fragmentId, owner)
    }

    return node
  },

  replaceText(node, value) {
    writeFragmentProp(node.canvasNodeId, node.fragmentId, "text", value)
    rendererInspectorStore.emit({ type: "text-changed", canvasNodeId: node.canvasNodeId, fragmentId: node.fragmentId, value })
    canvasFragmentRequestRepaint(node.canvasNodeId)
  },

  setProperty(node, name, value, prev) {
    patchFragmentProp(node, name, prev, value)
  },

  insertNode(parent, node, anchor) {
    parent.insertChild(node, anchor)
    const parentFid = parent.fragmentId === FRAGMENT_ROOT_ID ? null : parent.fragmentId
    rendererInspectorStore.emit({
      type: "node-inserted",
      canvasNodeId: parent.canvasNodeId,
      parentFragmentId: parentFid,
      childFragmentId: node.fragmentId,
      anchorFragmentId: anchor?.fragmentId ?? null,
    })
    canvasFragmentRequestRepaint(parent.canvasNodeId)
  },

  removeNode(parent, node) {
    cleanupHoverOnRemove(parent.canvasNodeId, node)
    parent.removeChild(node)
    const parentFid = parent.fragmentId === FRAGMENT_ROOT_ID ? null : parent.fragmentId
    rendererInspectorStore.emit({
      type: "node-removed",
      canvasNodeId: parent.canvasNodeId,
      parentFragmentId: parentFid,
      childFragmentId: node.fragmentId,
    })
    rendererInspectorStore.emit({ type: "node-destroyed", canvasNodeId: node.canvasNodeId, fragmentId: node.fragmentId })
    rendererInspectorStore.removeNode(node.canvasNodeId, node.fragmentId)
    node.destroy()
    canvasFragmentRequestRepaint(parent.canvasNodeId)
  },

  getParentNode(node) {
    return node.parent ?? undefined
  },

  getFirstChild(node) {
    return node.firstChild ?? undefined
  },

  getNextSibling(node) {
    return node.nextSibling ?? undefined
  },

  isTextNode(_node) {
    return false
  },
})

// ---------------------------------------------------------------------------
// Widget renderer — minimal, only for spread/setProp on NativeWidgetNode
// ---------------------------------------------------------------------------

const widgetRenderer = createRenderer<NativeWidgetNode>({
  createElement() { throw new Error("Use createNativeWidget() instead") },
  createTextNode() { throw new Error("Widget renderer does not support text nodes") },
  replaceText() {},
  setProperty(node, name, value, prev) { patchNativeProp(node, name, prev, value) },
  insertNode() {},
  removeNode() {},
  getParentNode() { return undefined },
  getFirstChild() { return undefined },
  getNextSibling() { return undefined },
  isTextNode() { return false },
})

// ---------------------------------------------------------------------------
// Prop patching
// ---------------------------------------------------------------------------

function patchNativeProp(node: NativeWidgetNode, key: string, prev: unknown, next: unknown): void {
  if (key === "ref") {
    if (typeof next === "function") next(node)
    return
  }

  if (key === QT_SOLID_SOURCE_META_PROP) {
    return
  }

  // Event props
  const exportId = WINDOW_EVENT_EXPORTS[key]
  if (exportId != null) {
    if (typeof next === "function") {
      const nodeWired = wiredEventExports.get(node.id) ?? new Set<number>()
      if (!nodeWired.has(exportId)) {
        nodeWired.add(exportId)
        wiredEventExports.set(node.id, nodeWired)
        node.qtNode.wireEvent(exportId)
      }
    }
    setNativeEventHandler(node.id, key, prev, next)
    return
  }

  // Prop reset
  if (next == null) {
    return
  }

  // Regular prop
  node.qtNode.applyProp({ prop: key, value: next } as any)
}

// ---------------------------------------------------------------------------
// Inline motion binding — lazy per-node state
// ---------------------------------------------------------------------------

const MOTION_PROP_KEYS_SET = new Set<string>(MOTION_PROP_KEYS as unknown as string[])

interface InlineMotionState {
  bag: Record<string, unknown>
  trigger: () => void
}

const inlineMotionStates = new WeakMap<FragmentRendererNode, InlineMotionState>()

function ensureInlineMotion(node: FragmentRendererNode): InlineMotionState {
  let state = inlineMotionStates.get(node)
  if (state) return state

  const bag: Record<string, unknown> = {}
  const [track, trigger] = createSignal(undefined, { equals: false })

  state = { bag, trigger }
  inlineMotionStates.set(node, state)

  // Gesture signals
  const [isHovered, setIsHovered] = createSignal(false)
  const [isTapped, setIsTapped] = createSignal(false)
  const [isFocused, setIsFocused] = createSignal(false)
  const [isDragging] = createSignal(false)

  const gesture: GestureState = { isHovered, isTapped, isFocused, isDragging }
  const dragCtrl: DragController = { onDown() {}, onMove() {}, onUp() {} }

  // Register gesture event handlers on the node's separate motion map
  node.motionGestureHandlers.set("onPointerEnter", () => setIsHovered(true))
  node.motionGestureHandlers.set("onPointerLeave", (() => { setIsHovered(false); setIsTapped(false) }) as () => void)
  node.motionGestureHandlers.set("onPointerDown", ((ev: unknown) => {
    setIsTapped(true)
    const { x, y } = ev as { x: number; y: number }
    dragCtrl.onDown(x, y)
  }) as (...args: unknown[]) => void)
  node.motionGestureHandlers.set("onPointerMove", ((ev: unknown) => {
    const { x, y } = ev as { x: number; y: number }
    dragCtrl.onMove(x, y)
  }) as (...args: unknown[]) => void)
  node.motionGestureHandlers.set("onPointerUp", ((ev: unknown) => {
    setIsTapped(false)
    const { x, y } = ev as { x: number; y: number }
    dragCtrl.onUp(x, y)
  }) as (...args: unknown[]) => void)
  node.motionGestureHandlers.set("onFocusIn", () => setIsFocused(true))
  node.motionGestureHandlers.set("onFocusOut", () => setIsFocused(false))

  // Bind motion — readMotion accessor reads from bag, reactivity via track()
  bindMotionNode(
    node as unknown as import("../app/motion/motion.ts").MotionNodeHandle,
    () => { track(); return bag as unknown as MotionComponentProps<object> },
    gesture,
    dragCtrl,
  )

  return state
}

// ---------------------------------------------------------------------------
// Layout prop system
// ---------------------------------------------------------------------------

const LAYOUT_PROPS = new Set([
  // Direction
  'row', 'column',
  // Sizing
  'w', 'h',
  // Alignment & distribution
  'align', 'spacing',
  // Gap, padding, margin
  'gap', 'padding', 'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft',
  'margin', 'marginTop', 'marginRight', 'marginBottom', 'marginLeft',
  // Constraints
  'minWidth', 'maxWidth', 'minHeight', 'maxHeight',
  // Absolute
  'absolute', 'top', 'right', 'bottom', 'left',
  // Layout visibility
  'layoutVisible',
  // Overflow & stacking
  'overflow', 'overflowX', 'overflowY', 'zIndex', 'wrap',
  // Grid child
  'gridRow', 'gridColumn', 'rowSpan', 'colSpan',
  // Grid container (on <grid> element)
  'columns', 'rows', 'columnGap', 'rowGap',
])

function isLayoutProp(key: string): boolean {
  return LAYOUT_PROPS.has(key)
}

// Parse align string: "center" → [v, h] both center; "top right" → [top, right]
function parseAlign(value: string, isRow: boolean): { primary: string; cross: string } {
  const parts = value.trim().split(/\s+/)
  let vertical: string
  let horizontal: string
  if (parts.length === 1) {
    vertical = parts[0]!
    horizontal = parts[0]!
  } else {
    vertical = parts[0]!
    horizontal = parts[1]!
  }

  // Map screen-space to primary/cross based on direction
  // row: primary = horizontal axis, cross = vertical axis
  // column: primary = vertical axis, cross = horizontal axis
  const vMap: Record<string, string> = { top: 'start', center: 'center', bottom: 'end', stretch: 'stretch' }
  const hMap: Record<string, string> = { left: 'start', center: 'center', right: 'end', stretch: 'stretch' }

  const mappedV = vMap[vertical] ?? 'start'
  const mappedH = hMap[horizontal] ?? 'start'

  if (isRow) {
    return { primary: mappedH, cross: mappedV }
  }
  return { primary: mappedV, cross: mappedH }
}

function writeLayoutProp(node: FragmentRendererNode, key: string, value: unknown): void {
  if (value == null) {
    canvasFragmentSetProp(node.canvasNodeId, node.fragmentId, key, { type: "unset" } as never)
    return
  }

  switch (key) {
    // ─── Direction ───
    case 'row':
      if (value === true) {
        node._direction = 'horizontal'
        writeFragmentProp(node.canvasNodeId, node.fragmentId, 'direction', 'horizontal')
      }
      break
    case 'column':
      if (value === true) {
        node._direction = 'vertical'
        writeFragmentProp(node.canvasNodeId, node.fragmentId, 'direction', 'vertical')
      }
      break

    // ─── Sizing ───
    case 'w':
    case 'h':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, key, value)
      break

    // ─── Alignment ───
    case 'align': {
      const isRow = node._direction === 'horizontal'
      const { primary, cross } = parseAlign(value as string, isRow)
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'primaryAlign', primary)
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'crossAlign', cross)
      break
    }

    // ─── Spacing distribution ───
    case 'spacing': {
      const spacingMap: Record<string, string> = {
        between: 'space-between',
        around: 'space-around',
        evenly: 'space-evenly',
      }
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'primaryAlign', spacingMap[value as string] ?? value)
      break
    }

    // ─── Gap ───
    case 'gap':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'gap', value)
      break

    // ─── Padding ───
    case 'padding':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutPadding', value)
      break
    case 'paddingTop':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutPaddingTop', value)
      break
    case 'paddingRight':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutPaddingRight', value)
      break
    case 'paddingBottom':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutPaddingBottom', value)
      break
    case 'paddingLeft':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutPaddingLeft', value)
      break

    // ─── Margin ───
    case 'margin':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutMargin', value)
      break
    case 'marginTop':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutMarginTop', value)
      break
    case 'marginRight':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutMarginRight', value)
      break
    case 'marginBottom':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutMarginBottom', value)
      break
    case 'marginLeft':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutMarginLeft', value)
      break

    // ─── Constraints ───
    case 'minWidth':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutMinWidth', value)
      break
    case 'maxWidth':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutMaxWidth', value)
      break
    case 'minHeight':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutMinHeight', value)
      break
    case 'maxHeight':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutMaxHeight', value)
      break

    // ─── Absolute positioning ───
    case 'absolute':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutPosition', value ? 'absolute' : 'relative')
      break
    case 'top':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutTop', value)
      break
    case 'right':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutRight', value)
      break
    case 'bottom':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutBottom', value)
      break
    case 'left':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutLeft', value)
      break

    // ─── Layout visibility ───
    case 'layoutVisible':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutVisible', value)
      break

    // ─── Overflow ───
    case 'overflow':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutOverflow', value)
      break
    case 'overflowX':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutOverflowX', value)
      break
    case 'overflowY':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutOverflowY', value)
      break

    // ─── zIndex ───
    case 'zIndex':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'zIndex', value)
      break

    // ─── Wrap ───
    case 'wrap':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'wrap', value)
      break

    // ─── Grid child ───
    case 'gridRow':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutGridRow', value)
      break
    case 'gridColumn':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutGridColumn', value)
      break
    case 'rowSpan':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutGridRowSpan', value)
      break
    case 'colSpan':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutGridColSpan', value)
      break

    // ─── Grid container ───
    case 'columns':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutGridColumns', {
        type: "gridtracks",
        tracks: (value as Array<number | string>).map((t) => typeof t === "number" ? String(t) : t),
      })
      break
    case 'rows':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutGridRows', {
        type: "gridtracks",
        tracks: (value as Array<number | string>).map((t) => typeof t === "number" ? String(t) : t),
      })
      break
    case 'columnGap':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutGridColumnGap', value)
      break
    case 'rowGap':
      writeFragmentProp(node.canvasNodeId, node.fragmentId, 'layoutGridRowGap', value)
      break

    default:
      writeFragmentProp(node.canvasNodeId, node.fragmentId, key, value)
  }
}

// ---------------------------------------------------------------------------
// Prop patching — fragment nodes
// ---------------------------------------------------------------------------

function patchFragmentProp(node: FragmentRendererNode, key: string, _prev: unknown, next: unknown): void {
  if (key === "ref") {
    if (typeof next === "function") next(node)
    return
  }

  if (key === QT_SOLID_SOURCE_META_PROP) {
    if (isQtSolidSourceMetadata(next)) {
      rendererInspectorStore.setSource(node.canvasNodeId, node.fragmentId, next)
    } else {
      rendererInspectorStore.clearSource(node.canvasNodeId, node.fragmentId)
    }
    return
  }

  // Motion prop interception
  if (MOTION_PROP_KEYS_SET.has(key)) {
    const state = ensureInlineMotion(node)
    state.bag[key] = next
    state.trigger()
    return
  }

  // Mask prop: render mask element as child, tell native it's a mask
  if (key === "mask") {
    const maskNode = next as FragmentRendererNode | null
    if (maskNode) {
      node.insertChild(maskNode)
      canvasFragmentSetF64Prop(node.canvasNodeId, node.fragmentId, "maskChild", maskNode.fragmentId)
    } else {
      canvasFragmentSetProp(node.canvasNodeId, node.fragmentId, "maskChild", { type: "unset" } as never)
    }
    return
  }

  if (HANDLED_EVENT_NAMES.has(key)) {
    if (typeof next === "function") {
      node.eventHandlers.set(key, next as (...args: unknown[]) => void)
    } else {
      node.eventHandlers.delete(key)
    }
    if (key === "onLayout") {
      canvasFragmentSetListener(
        node.canvasNodeId,
        node.fragmentId,
        FRAGMENT_LISTENER_LAYOUT,
        typeof next === "function",
      )
    }
    return
  }

  // Figma layout intent interception
  if (LAYOUT_PROPS.has(key)) {
    node._usesIntentLayout = true
  }

  if (node._usesIntentLayout && isLayoutProp(key)) {
    writeLayoutProp(node, key, next)
    canvasFragmentRequestRepaint(node.canvasNodeId)
    return
  }

  // Transform prop rename: transformX/transformY → native x/y
  if (key === "transformX") {
    if (next == null) {
      canvasFragmentSetProp(node.canvasNodeId, node.fragmentId, "x", { type: "unset" } as never)
    } else {
      writeFragmentProp(node.canvasNodeId, node.fragmentId, "x", next)
    }
    canvasFragmentRequestRepaint(node.canvasNodeId)
    return
  }
  if (key === "transformY") {
    if (next == null) {
      canvasFragmentSetProp(node.canvasNodeId, node.fragmentId, "y", { type: "unset" } as never)
    } else {
      writeFragmentProp(node.canvasNodeId, node.fragmentId, "y", next)
    }
    canvasFragmentRequestRepaint(node.canvasNodeId)
    return
  }

  if (next == null) {
    canvasFragmentSetProp(node.canvasNodeId, node.fragmentId, key, { type: "unset" } as never)
  } else {
    writeFragmentProp(node.canvasNodeId, node.fragmentId, key, next)
  }
  canvasFragmentRequestRepaint(node.canvasNodeId)
  rendererInspectorStore.emit({ type: "prop-changed", canvasNodeId: node.canvasNodeId, fragmentId: node.fragmentId, key })
}

// ---------------------------------------------------------------------------
// Fragment renderer exports (solid-js/universal API)
// ---------------------------------------------------------------------------

export const {
  render: _render,
  effect,
  memo,
  createElement,
  createTextNode,
  insertNode,
  insert,
  spread,
  setProp,
  mergeProps,
  use,
} = fragmentRenderer

const createComponentBase = fragmentRenderer.createComponent

function hasAnyMotionProp(props: Record<string, unknown>): boolean {
  for (const key of MOTION_PROP_KEYS) {
    if (key in props) return true
  }
  return false
}

export const createComponent = ((...args: Parameters<typeof createComponentBase>) => {
  const [component, props] = args

  if (!hasAnyMotionProp(props as Record<string, unknown>)) {
    return withQtOwnerFrame(component, props, () => createComponentBase(...args))
  }

  // Split motion props from component props
  const motionBag: Record<string, unknown> = {}
  const baseProps: Record<string, unknown> = {}
  const descriptors = Object.getOwnPropertyDescriptors(props)
  for (const key of Object.keys(descriptors)) {
    if (MOTION_PROP_KEYS_SET.has(key)) {
      // Copy descriptor so reactive getters keep working
      Object.defineProperty(motionBag, key, descriptors[key]!)
    } else {
      Object.defineProperty(baseProps, key, descriptors[key]!)
    }
  }

  const element = withQtOwnerFrame(component, baseProps, () =>
    createComponentBase(component, baseProps),
  )

  if (isMotionNodeHandle(element)) {
    const [track, trigger] = createSignal(undefined, { equals: false })
    const [isHovered, setIsHovered] = createSignal(false)
    const [isTapped, setIsTapped] = createSignal(false)
    const [isFocused, setIsFocused] = createSignal(false)
    const [isDragging] = createSignal(false)
    const gesture: GestureState = { isHovered, isTapped, isFocused, isDragging }
    const dragCtrl: DragController = { onDown() {}, onMove() {}, onUp() {} }

    const fragNode = element as unknown as FragmentRendererNode
    fragNode.motionGestureHandlers.set("onPointerEnter", () => setIsHovered(true))
    fragNode.motionGestureHandlers.set("onPointerLeave", (() => { setIsHovered(false); setIsTapped(false) }) as () => void)
    fragNode.motionGestureHandlers.set("onPointerDown", ((ev: unknown) => {
      setIsTapped(true)
      const { x, y } = ev as { x: number; y: number }
      dragCtrl.onDown(x, y)
    }) as (...args: unknown[]) => void)
    fragNode.motionGestureHandlers.set("onPointerMove", ((ev: unknown) => {
      const { x, y } = ev as { x: number; y: number }
      dragCtrl.onMove(x, y)
    }) as (...args: unknown[]) => void)
    fragNode.motionGestureHandlers.set("onPointerUp", ((ev: unknown) => {
      setIsTapped(false)
      const { x, y } = ev as { x: number; y: number }
      dragCtrl.onUp(x, y)
    }) as (...args: unknown[]) => void)
    fragNode.motionGestureHandlers.set("onFocusIn", () => setIsFocused(true))
    fragNode.motionGestureHandlers.set("onFocusOut", () => setIsFocused(false))

    bindMotionNode(
      element,
      () => { track(); return motionBag as unknown as MotionComponentProps<object> },
      gesture,
      dragCtrl,
    )

    // Trigger once so initial reactive tracking is established
    trigger()
  }

  return element
}) as typeof fragmentRenderer.createComponent

// ---------------------------------------------------------------------------
// Widget renderer exports
// ---------------------------------------------------------------------------

export const spreadWidgetProps = widgetRenderer.spread
export const setWidgetProp = widgetRenderer.setProp

// ---------------------------------------------------------------------------
// Re-exports from submodules for public API
// ---------------------------------------------------------------------------

export { FragmentRendererNode, createCanvasFragmentBinding } from "./fragment.ts"
export { registerCanvasBinding, unregisterCanvasBinding, destroyCanvasFragmentBinding } from "./canvas/registry.ts"
export { dispatchCanvasPointerEvent, dispatchCanvasPointerMoveForHover, dispatchCanvasMotionComplete, dispatchCanvasFocusChange, dispatchCanvasTextInputChange, dispatchCanvasKeyboardEvent, dispatchCanvasWheelEvent, dispatchFragmentLayout, HANDLED_EVENT_NAMES } from "./canvas/dispatch.ts"
export { handleEvent, fileDialogChannel, onColorSchemeChange, onScreenDpiChange } from "./host-events.ts"
