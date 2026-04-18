import {
  __qtSolidTraceEnterInteraction,
  __qtSolidTraceExitInteraction,
  __qtSolidTraceRecordJs,
  type QtApp,
  type QtHostEvent,
  type QtNode,
} from "@qt-solid/core"
import type { QtApp as RawQtApp } from "@qt-solid/core/native"
import {
  resolveQtWidgetLibraryEntryForIntrinsic,
  type IntrinsicNode,
  type QtWidgetLibraryEntry,
} from "@qt-solid/core/widget-library"
import { rendererInspectorStore } from "../devtools/inspector-store.ts"
import type { QtSolidOwnerMetadata } from "../devtools/owner-metadata.ts"
import { isQtSolidSourceMetadata, QT_SOLID_SOURCE_META_PROP } from "../devtools/source-metadata.ts"
import type { QtRendererBinding } from "./renderer.ts"

type NativeEventHandler = (...args: unknown[]) => void
type BoundIntrinsicNode = {
  intrinsic: IntrinsicNode
  libraryEntry: QtWidgetLibraryEntry
}

type EventBatchEvent = Extract<QtHostEvent, { type: "listenerbatch" }>

export interface NativeQtRendererBinding extends QtRendererBinding<QtNode> {
  handleEvent(event: QtHostEvent): void
}

function dispatchEventBatch(
  libraryEntry: QtWidgetLibraryEntry,
  emit: (nodeId: number, key: string, ...args: unknown[]) => void,
  event: EventBatchEvent,
): void {
  for (const listenerId of event.listenerIds) {
    libraryEntry.library.dispatchNativeEvent(emit, {
      type: "listener",
      nodeId: event.nodeId,
      listenerId,
      traceId: event.traceId,
      values: event.values,
    })
  }
}

function collectEventBatchControlledValues(
  libraryEntry: QtWidgetLibraryEntry,
  event: EventBatchEvent,
) {
  const values = []
  for (const listenerId of event.listenerIds) {
    values.push(
      ...libraryEntry.library.collectControlledPropValues({
        type: "listener",
        nodeId: event.nodeId,
        listenerId,
        traceId: event.traceId,
        values: event.values,
      }),
    )
  }
  return values
}

export function createNativeRendererBinding(app: QtApp): NativeQtRendererBinding {
  const canonicalNodes = new Map<number, QtNode>()
  const intrinsicNodes = new Map<number, BoundIntrinsicNode>()
  const eventHandlerMap = new Map<number, Map<string, Set<NativeEventHandler>>>()
  const traceStack: number[] = []
  const controlledPropValues = new Map<number, Map<string, unknown>>()
  const rootId = app.root.id

  rendererInspectorStore.reset(rootId)

  const intrinsicOf = (nodeId: number): BoundIntrinsicNode | undefined => intrinsicNodes.get(nodeId)

  const currentTraceId = (): number | undefined => traceStack[traceStack.length - 1]

  const traceJs = (
    stage: string,
    nodeId?: number,
    listenerId?: number,
    propId?: number,
    detail?: string,
  ) => {
    const traceId = currentTraceId()
    if (traceId == null) {
      return
    }

    __qtSolidTraceRecordJs(traceId, stage, nodeId, listenerId, propId, detail)
  }

  const canon = (node: QtNode | null | undefined): QtNode | undefined => {
    if (!node) {
      return undefined
    }

    const existing = canonicalNodes.get(node.id)
    if (existing) {
      return existing
    }

    canonicalNodes.set(node.id, node)
    return node
  }

  const forgetSubtree = (node: QtNode | null | undefined) => {
    if (!node) {
      return
    }

    let child = canon(node.firstChild)
    while (child) {
      const next = canon(child.nextSibling)
      forgetSubtree(child)
      child = next
    }

    canonicalNodes.delete(node.id)
    intrinsicNodes.delete(node.id)
    eventHandlerMap.delete(node.id)
    controlledPropValues.delete(node.id)
  }

  const setEventHandler = (node: QtNode, key: string, prev: unknown, next: unknown) => {
    const nodeListeners = eventHandlerMap.get(node.id)

    if (typeof prev === "function" && nodeListeners) {
      const listeners = nodeListeners.get(key)
      listeners?.delete(prev as NativeEventHandler)
      if (listeners?.size === 0) {
        nodeListeners.delete(key)
      }
      if (nodeListeners.size === 0) {
        eventHandlerMap.delete(node.id)
      }
    }

    if (typeof next !== "function") {
      return
    }

    const ensuredNodeListeners = eventHandlerMap.get(node.id) ?? new Map<string, Set<NativeEventHandler>>()
    const listeners = ensuredNodeListeners.get(key) ?? new Set<NativeEventHandler>()
    listeners.add(next as NativeEventHandler)
    ensuredNodeListeners.set(key, listeners)
    eventHandlerMap.set(node.id, ensuredNodeListeners)
  }

  const emit = (nodeId: number, key: string, ...args: unknown[]) => {
    const listeners = eventHandlerMap.get(nodeId)?.get(key)
    if (!listeners) {
      return
    }

    const intrinsic = intrinsicOf(nodeId)
    const listenerId = intrinsic?.libraryEntry.library.eventExportIds[key]
    for (const listener of listeners) {
      traceJs("js.listener.enter", nodeId, listenerId)
      listener(...args)
      traceJs("js.listener.exit", nodeId, listenerId)
    }
  }

  const shouldSkipControlledPropWrite = (node: QtNode, leafKey: string, next: unknown): boolean => {
    const nodeValues = controlledPropValues.get(node.id)
    if (!nodeValues) {
      return false
    }

    if (nodeValues.get(leafKey) === next) {
      nodeValues.delete(leafKey)
      if (nodeValues.size === 0) {
        controlledPropValues.delete(node.id)
      }
      return true
    }

    return false
  }

  return {
    root: canon(app.root)!,
    createElement(type) {
      const widgetLibraryEntry = resolveQtWidgetLibraryEntryForIntrinsic(type)
      const intrinsicNode = widgetLibraryEntry.library.createIntrinsicNode(
        widgetLibraryEntry.nativeBridge,
        app as unknown as RawQtApp,
        type,
      )
      if (!intrinsicNode) {
        throw new Error(`Qt intrinsic node is missing for intrinsic ${type}`)
      }
      const node = canon(intrinsicNode.node)!
      intrinsicNodes.set(node.id, { intrinsic: intrinsicNode, libraryEntry: widgetLibraryEntry })
      rendererInspectorStore.ensureElementNode(node.id, type)
      return node
    },
    createTextNode(value) {
      const widgetLibraryEntry = resolveQtWidgetLibraryEntryForIntrinsic("text")
      const intrinsicNode = widgetLibraryEntry.library.createIntrinsicNode(
        widgetLibraryEntry.nativeBridge,
        app as unknown as RawQtApp,
        "text",
      )
      if (!intrinsicNode) {
        throw new Error("Qt intrinsic text node is missing")
      }
      const node = canon(intrinsicNode.node)!
      intrinsicNodes.set(node.id, { intrinsic: intrinsicNode, libraryEntry: widgetLibraryEntry })
      intrinsicNode.applyProp("text", undefined, value)
      rendererInspectorStore.ensureTextNode(node.id, value)
      return node
    },
    attachDebugMetadata(node, metadata) {
      const owner = metadata.owner as QtSolidOwnerMetadata | null | undefined
      if (owner) {
        rendererInspectorStore.setOwner(node.id, owner)
        return
      }

      rendererInspectorStore.clearOwner(node.id)
    },
    replaceText(node, value) {
      const intrinsic = intrinsicNodes.get(node.id)
      if (!intrinsic) {
        throw new Error(`Qt intrinsic node is missing for text node ${node.id}`)
      }
      intrinsic.intrinsic.applyProp("text", undefined, value)
      rendererInspectorStore.replaceText(node.id, value)
    },
    insertChild(parent, child, anchor) {
      intrinsicNodes.get(child.id)?.intrinsic.finalizeMount()
      parent.insertChild(child, anchor ?? null)
      rendererInspectorStore.insertChild(parent.id, child.id, anchor?.id)
    },
    removeChild(parent, child) {
      forgetSubtree(child)
      rendererInspectorStore.removeChild(parent.id, child.id)
      if (parent.id !== rootId) {
        parent.removeChild(child)
      }
      child.destroy()
      rendererInspectorStore.destroySubtree(child.id)
    },
    getParent(node) {
      return canon(node.parent)
    },
    getFirstChild(node) {
      return canon(node.firstChild)
    },
    getNextSibling(node) {
      return canon(node.nextSibling)
    },
    isTextNode(node) {
      return node.isTextNode()
    },
    patchProp(node, key, prev, next) {
      const intrinsic = intrinsicNodes.get(node.id)

      if (key === QT_SOLID_SOURCE_META_PROP) {
        if (isQtSolidSourceMetadata(next)) {
          rendererInspectorStore.setSource(node.id, next)
        } else {
          rendererInspectorStore.clearSource(node.id)
        }
        return
      }

      if (!intrinsic) {
        throw new Error(`Qt intrinsic node is missing for node ${node.id}`)
      }

      const widgetLibrary = intrinsic.libraryEntry.library

      if (widgetLibrary.isEventExportProp(key)) {
        setEventHandler(node, key, prev, next)
        rendererInspectorStore.setListener(node.id, key, typeof next === "function")
        return
      }

      if (next == null) {
        const propId = intrinsic.intrinsic.propIdForKey(key)
        traceJs("js.patch_prop.enter", node.id, undefined, propId, `${key}:reset`)
        if (
          !intrinsic.intrinsic.applyProp(
            key,
            prev,
            undefined,
            (leafKey, leafValue) => {
              const skip = shouldSkipControlledPropWrite(node, leafKey, leafValue)
              if (skip) {
                const leafPropId = intrinsic.intrinsic.propIdForKey(leafKey)
                traceJs("js.patch_prop.skip_controlled_echo", node.id, undefined, leafPropId, leafKey)
              }
              return skip
            },
          )
        ) {
          throw new Error(`Unsupported intrinsic prop reset: ${key}`)
        }
        traceJs("js.patch_prop.exit", node.id, undefined, propId, `${key}:reset`)
        rendererInspectorStore.clearProp(node.id, key)
        return
      }

      const propId = intrinsic.intrinsic.propIdForKey(key)
      traceJs("js.patch_prop.enter", node.id, undefined, propId, key)
      if (
        !intrinsic.intrinsic.applyProp(
          key,
          prev,
          next,
          (leafKey, leafValue) => {
            const skip = shouldSkipControlledPropWrite(node, leafKey, leafValue)
            if (skip) {
              const leafPropId = intrinsic.intrinsic.propIdForKey(leafKey)
              traceJs("js.patch_prop.skip_controlled_echo", node.id, undefined, leafPropId, leafKey)
            }
            return skip
          },
        )
      ) {
        throw new Error(`Unsupported intrinsic prop: ${key}`)
      }
      traceJs("js.patch_prop.exit", node.id, undefined, propId, key)
      rendererInspectorStore.setProp(node.id, key, next)
    },
    handleEvent(event) {
      const listenerLike =
        event.type === "listener" || event.type === "listenerbatch"
      const intrinsic = listenerLike ? intrinsicOf(event.nodeId) : undefined
      if (listenerLike && intrinsic) {
        const nextValues =
          event.type === "listenerbatch"
            ? collectEventBatchControlledValues(intrinsic.libraryEntry, event)
            : intrinsic.libraryEntry.library.collectControlledPropValues(event)
        if (nextValues.length > 0) {
          const nodeValues = controlledPropValues.get(event.nodeId) ?? new Map<string, unknown>()
          for (const entry of nextValues) {
            nodeValues.set(entry.propKey, entry.value)
          }
          controlledPropValues.set(event.nodeId, nodeValues)
        }
      }

      if (listenerLike && event.traceId != null) {
        traceStack.push(event.traceId)
        __qtSolidTraceEnterInteraction(event.traceId)
        __qtSolidTraceRecordJs(
          event.traceId,
          "js.handle_event.enter",
          event.nodeId,
          event.type === "listener" ? event.listenerId : undefined,
          undefined,
          undefined,
        )
        try {
          if (!intrinsic) {
            throw new Error(`Qt intrinsic node is missing for event node ${event.nodeId}`)
          }
          if (event.type === "listenerbatch") {
            dispatchEventBatch(intrinsic.libraryEntry, emit, event)
          } else {
            intrinsic.libraryEntry.library.dispatchNativeEvent(emit, event)
          }
          __qtSolidTraceRecordJs(
            event.traceId,
            "js.handle_event.exit",
            event.nodeId,
            event.type === "listener" ? event.listenerId : undefined,
            undefined,
            undefined,
          )
        } finally {
          __qtSolidTraceExitInteraction()
          traceStack.pop()
        }
        return
      }

      if (event.type === "listener") {
        if (!intrinsic) {
          throw new Error(`Qt intrinsic node is missing for event node ${event.nodeId}`)
        }
        intrinsic.libraryEntry.library.dispatchNativeEvent(emit, event)
      } else if (event.type === "listenerbatch") {
        if (!intrinsic) {
          throw new Error(`Qt intrinsic node is missing for event node ${event.nodeId}`)
        }
        dispatchEventBatch(intrinsic.libraryEntry, emit, event)
      }
    },
  }
}
