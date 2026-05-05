import { createContext, useContext } from "solid-js"
import { createRenderer } from "solid-js/universal"

import {
  type QtApp,
  type QtNode,
} from "@qt-solid/core"
import {
  canvasFragmentCreate,
  canvasFragmentRequestRepaint,
  canvasFragmentSetProp,
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

export const createComponent = ((...args: Parameters<typeof createComponentBase>) => {
  const [component, props] = args
  return withQtOwnerFrame(component, props, () => createComponentBase(...args))
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
