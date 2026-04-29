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
import type { QtSolidOwnerMetadata } from "../devtools/owner-metadata.ts"
import { isQtSolidSourceMetadata, QT_SOLID_SOURCE_META_PROP } from "../devtools/source-metadata.ts"
import { FRAGMENT_ROOT_ID, FragmentRendererNode, writeFragmentProp } from "./fragment.ts"
import { HANDLED_EVENT_NAMES } from "./canvas/dispatch.ts"
import { cleanupHoverOnRemove } from "./canvas/registry.ts"
import {
  WINDOW_EVENT_EXPORTS,
  wiredEventExports,
  setNativeEventHandler,
  forgetNativeEvents,
  traceJs,
} from "./host-events.ts"

const FRAGMENT_LISTENER_LAYOUT = 1

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export type QtFlexDirection = "column" | "row"
export type QtAlignItems = "flex-start" | "center" | "flex-end" | "stretch"
export type QtJustifyContent = "flex-start" | "center" | "flex-end"

export interface QtRendererNode {
  readonly id: number
  readonly nodeKind: "native" | "fragment"
  readonly parent: QtRendererNode | null
  readonly firstChild: QtRendererNode | null
  readonly nextSibling: QtRendererNode | null
  isTextNode(): boolean
  insertChild(child: QtRendererNode, anchor?: QtRendererNode | null): void
  removeChild(child: QtRendererNode): void
  destroy(): void
}

export interface QtRendererDebugMetadata {
  owner?: QtSolidOwnerMetadata | null
}

// ---------------------------------------------------------------------------
// Canvas scope context — provided by Window/Canvas to children
// ---------------------------------------------------------------------------

export interface CanvasScope {
  readonly canvasNodeId: number
  readonly root: FragmentRendererNode
}

export const CanvasScopeContext = createContext<CanvasScope | null>(null)

// ---------------------------------------------------------------------------
// Native widget node — wraps napi QtNode for windows
// ---------------------------------------------------------------------------

class NativeWidgetNode implements QtRendererNode {
  readonly nodeKind = "native" as const
  readonly qtNode: QtNode

  constructor(qtNode: QtNode) {
    this.qtNode = qtNode
  }

  get id(): number {
    return this.qtNode.id
  }

  get parent(): QtRendererNode | null {
    const p = this.qtNode.parent
    return p ? canonNode(p) ?? null : null
  }

  get firstChild(): QtRendererNode | null {
    const c = this.qtNode.firstChild
    return c ? canonNode(c) ?? null : null
  }

  get nextSibling(): QtRendererNode | null {
    const s = this.qtNode.nextSibling
    return s ? canonNode(s) ?? null : null
  }

  isTextNode(): boolean {
    return this.qtNode.isTextNode()
  }

  insertChild(child: QtRendererNode, anchor?: QtRendererNode | null): void {
    if (child.nodeKind === "fragment") {
      throw new Error("Cannot insert fragment node into native widget — use CanvasScope root")
    }
    const nativeChild = child as NativeWidgetNode
    const nativeAnchor = anchor ? (anchor as NativeWidgetNode) : null
    this.qtNode.insertChild(nativeChild.qtNode, nativeAnchor?.qtNode ?? null)
  }

  removeChild(child: QtRendererNode): void {
    if (child.nodeKind === "fragment") {
      throw new Error("Cannot remove fragment node from native widget")
    }
    const nativeChild = child as NativeWidgetNode
    this.qtNode.removeChild(nativeChild.qtNode)
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

function canonNode(qtNode: QtNode): NativeWidgetNode | undefined {
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

// ---------------------------------------------------------------------------
// Solid.js universal renderer
// ---------------------------------------------------------------------------

function useCanvasScope(): CanvasScope {
  const scope = useContext(CanvasScopeContext)
  if (!scope) {
    throw new Error("Canvas scope required — createElement for fragments must be inside a Window or Canvas")
  }
  return scope
}

const renderer = createRenderer<QtRendererNode>({
  createElement(type) {
    if (type === "window" || type === "canvas") {
      // Native widget creation
      if (!currentApp) {
        throw new Error("Renderer not initialized — call initRenderer(app) first")
      }
      const qtNode = currentApp.createWidget()
      const node = new NativeWidgetNode(qtNode)
      canonicalNodes.set(qtNode.id, node)

      return node
    }

    // Fragment creation — requires canvas scope
    const scope = useCanvasScope()
    const fragmentId = canvasFragmentCreate(scope.canvasNodeId, type)
    const node = new FragmentRendererNode(scope.canvasNodeId, fragmentId, type)

    rendererInspectorStore.addCanvas(scope.canvasNodeId)
    rendererInspectorStore.emit({ type: "node-created", canvasNodeId: scope.canvasNodeId, fragmentId, kind: type })

    const owner = currentQtSolidOwnerMetadata()
    if (owner) {
      rendererInspectorStore.setOwner(scope.canvasNodeId, fragmentId, owner)
    }

    return node
  },

  createTextNode(value) {
    const scope = useCanvasScope()
    const fragmentId = canvasFragmentCreate(scope.canvasNodeId, "Text")
    const node = new FragmentRendererNode(scope.canvasNodeId, fragmentId, "Text")

    rendererInspectorStore.addCanvas(scope.canvasNodeId)
    rendererInspectorStore.emit({ type: "node-created", canvasNodeId: scope.canvasNodeId, fragmentId, kind: "#text" })
    writeFragmentProp(scope.canvasNodeId, fragmentId, "text", String(value))

    const owner = currentQtSolidOwnerMetadata()
    if (owner) {
      rendererInspectorStore.setOwner(scope.canvasNodeId, fragmentId, owner)
    }

    return node
  },

  replaceText(node, value) {
    if (node.nodeKind === "native") {
      const nw = node as NativeWidgetNode
      nw.qtNode.applyProp({ prop: "text", value } as any)
    } else {
      const fn = node as FragmentRendererNode
      writeFragmentProp(fn.canvasNodeId, fn.fragmentId, "text", value)
      rendererInspectorStore.emit({ type: "text-changed", canvasNodeId: fn.canvasNodeId, fragmentId: fn.fragmentId, value })
      canvasFragmentRequestRepaint(fn.canvasNodeId)
    }
  },

  setProperty(node, name, value, prev) {
    if (node.nodeKind === "native") {
      patchNativeProp(node as NativeWidgetNode, name, prev, value)
    } else {
      patchFragmentProp(node as FragmentRendererNode, name, prev, value)
    }
  },

  insertNode(parent, node, anchor) {
    if (parent.nodeKind === "native" && node.nodeKind === "native") {
      const nParent = parent as NativeWidgetNode
      const nChild = node as NativeWidgetNode
      const nAnchor = anchor ? (anchor as NativeWidgetNode) : undefined
      nParent.qtNode.insertChild(nChild.qtNode, nAnchor?.qtNode ?? null)
    } else if (parent.nodeKind === "fragment") {
      parent.insertChild(node, anchor)
      const fParent = parent as FragmentRendererNode
      const fChild = node as FragmentRendererNode
      const parentFid = fParent.fragmentId === FRAGMENT_ROOT_ID ? null : fParent.fragmentId
      rendererInspectorStore.emit({
        type: "node-inserted",
        canvasNodeId: fParent.canvasNodeId,
        parentFragmentId: parentFid,
        childFragmentId: fChild.fragmentId,
        anchorFragmentId: anchor ? (anchor as FragmentRendererNode).fragmentId : null,
      })
      canvasFragmentRequestRepaint(fParent.canvasNodeId)
    } else {
      throw new Error("Cannot insert fragment node into native widget — use CanvasScope root")
    }
  },

  removeNode(parent, node) {
    if (parent.nodeKind === "native" && node.nodeKind === "native") {
      const nParent = parent as NativeWidgetNode
      const nChild = node as NativeWidgetNode
      forgetNativeSubtree(nChild)
      if (nParent.id !== rootNode?.id) {
        nParent.qtNode.removeChild(nChild.qtNode)
      }
      nChild.destroy()
    } else if (parent.nodeKind === "fragment") {
      const fParent = parent as FragmentRendererNode
      const fChild = node as FragmentRendererNode
      cleanupHoverOnRemove(fParent.canvasNodeId, fChild)
      fParent.removeChild(fChild)
      const parentFid = fParent.fragmentId === FRAGMENT_ROOT_ID ? null : fParent.fragmentId
      rendererInspectorStore.emit({
        type: "node-removed",
        canvasNodeId: fParent.canvasNodeId,
        parentFragmentId: parentFid,
        childFragmentId: fChild.fragmentId,
      })
      rendererInspectorStore.emit({ type: "node-destroyed", canvasNodeId: fChild.canvasNodeId, fragmentId: fChild.fragmentId })
      rendererInspectorStore.removeNode(fChild.canvasNodeId, fChild.fragmentId)
      fChild.destroy()
      canvasFragmentRequestRepaint(fParent.canvasNodeId)
    } else {
      throw new Error("Cannot remove fragment node from native widget")
    }
  },

  getParentNode(node) {
    if (node.nodeKind === "native") {
      const p = (node as NativeWidgetNode).qtNode.parent
      return p ? canonNode(p) : undefined
    }
    return (node as FragmentRendererNode).parent ?? undefined
  },

  getFirstChild(node) {
    if (node.nodeKind === "native") {
      const c = (node as NativeWidgetNode).qtNode.firstChild
      return c ? canonNode(c) : undefined
    }
    return (node as FragmentRendererNode).firstChild ?? undefined
  },

  getNextSibling(node) {
    if (node.nodeKind === "native") {
      const s = (node as NativeWidgetNode).qtNode.nextSibling
      return s ? canonNode(s) : undefined
    }
    return (node as FragmentRendererNode).nextSibling ?? undefined
  },

  isTextNode(node) {
    if (node.nodeKind === "native") {
      return (node as NativeWidgetNode).qtNode.isTextNode()
    }
    return false
  },
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
// Renderer exports (solid-js/universal API)
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
} = renderer

const createComponentBase = renderer.createComponent

export const createComponent = ((...args: Parameters<typeof createComponentBase>) => {
  const [component, props] = args
  return withQtOwnerFrame(component, props, () => createComponentBase(...args))
}) as typeof renderer.createComponent

// ---------------------------------------------------------------------------
// Re-exports from submodules for public API
// ---------------------------------------------------------------------------

export { FragmentRendererNode, createCanvasFragmentBinding } from "./fragment.ts"
export { registerCanvasBinding, unregisterCanvasBinding, destroyCanvasFragmentBinding } from "./canvas/registry.ts"
export { dispatchCanvasPointerEvent, dispatchCanvasPointerMoveForHover, dispatchCanvasMotionComplete, dispatchCanvasFocusChange, dispatchCanvasTextInputChange, dispatchCanvasKeyboardEvent, dispatchCanvasWheelEvent, dispatchFragmentLayout, HANDLED_EVENT_NAMES } from "./canvas/dispatch.ts"
export { handleEvent, fileDialogChannel, onColorSchemeChange, onScreenDpiChange } from "./host-events.ts"
