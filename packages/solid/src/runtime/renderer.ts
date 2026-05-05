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
  let child = node.qtNode.firstChild
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
