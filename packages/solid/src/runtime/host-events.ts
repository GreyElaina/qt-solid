import type { QtHostEvent } from "@qt-solid/core"
import {
  traceEnterInteraction,
  traceExitInteraction,
  traceRecordJs,
} from "@qt-solid/core/native"

import {
  dispatchCanvasPointerEvent,
  dispatchCanvasPointerMoveForHover,
  dispatchCanvasMotionComplete,
  dispatchCanvasFocusChange,
  dispatchCanvasKeyboardEvent,
  dispatchCanvasTextInputChange,
  dispatchCanvasWheelEvent,
  dispatchFragmentLayout,
} from "./canvas/dispatch.ts"
import { hoveredFragments } from "./canvas/registry.ts"

// ---------------------------------------------------------------------------
// Event channel
// ---------------------------------------------------------------------------

function createEventChannel<T>() {
  const listeners = new Set<(value: T) => void>()
  return {
    subscribe(fn: (value: T) => void): () => void {
      listeners.add(fn)
      return () => { listeners.delete(fn) }
    },
    emit(value: T): void {
      listeners.forEach(fn => fn(value))
    },
  }
}

// ---------------------------------------------------------------------------
// Module-level state for native events
// ---------------------------------------------------------------------------

type NativeEventHandler = (...args: unknown[]) => void
export const eventHandlerMap = new Map<number, Map<string, Set<NativeEventHandler>>>()
export const wiredEventExports = new Map<number, Set<number>>()
const traceStack: number[] = []

export const WINDOW_EVENT_EXPORTS: Record<string, number> = {
  onCloseRequested: 1,
  onHoverEnter: 2,
  onHoverLeave: 3,
}

// Global app-level event channels
const colorSchemeChannel = createEventChannel<string>()
const screenDpiChannel = createEventChannel<number>()
export const fileDialogChannel = createEventChannel<{ requestId: number; paths: string[] }>()

export const onColorSchemeChange = colorSchemeChannel.subscribe
export const onScreenDpiChange = screenDpiChannel.subscribe

// ---------------------------------------------------------------------------
// Trace helpers
// ---------------------------------------------------------------------------

export const currentTraceId = (): number | undefined => traceStack[traceStack.length - 1]

export function traceJs(
  stage: string,
  nodeId?: number,
  listenerId?: number,
  propId?: number,
  detail?: string,
) {
  const traceId = currentTraceId()
  if (traceId == null) return
  traceRecordJs(traceId, stage, nodeId, listenerId, propId, detail)
}

// ---------------------------------------------------------------------------
// Native event handler management
// ---------------------------------------------------------------------------

export function setNativeEventHandler(nodeId: number, key: string, prev: unknown, next: unknown): void {
  const nodeListeners = eventHandlerMap.get(nodeId)

  if (typeof prev === "function" && nodeListeners) {
    const listeners = nodeListeners.get(key)
    listeners?.delete(prev as NativeEventHandler)
    if (listeners?.size === 0) nodeListeners.delete(key)
    if (nodeListeners.size === 0) eventHandlerMap.delete(nodeId)
  }

  if (typeof next !== "function") return

  const ensuredNodeListeners = eventHandlerMap.get(nodeId) ?? new Map<string, Set<NativeEventHandler>>()
  const listeners = ensuredNodeListeners.get(key) ?? new Set<NativeEventHandler>()
  listeners.add(next as NativeEventHandler)
  ensuredNodeListeners.set(key, listeners)
  eventHandlerMap.set(nodeId, ensuredNodeListeners)
}

export function emitNativeEvent(nodeId: number, key: string, ...args: unknown[]): void {
  const listeners = eventHandlerMap.get(nodeId)?.get(key)
  if (!listeners) return

  const listenerId = WINDOW_EVENT_EXPORTS[key]
  for (const listener of listeners) {
    traceJs("js.listener.enter", nodeId, listenerId)
    listener(...args)
    traceJs("js.listener.exit", nodeId, listenerId)
  }
}

// ---------------------------------------------------------------------------
// forgetNativeEvents — cleanup event state for a single node
// ---------------------------------------------------------------------------

export function forgetNativeEvents(nodeId: number): void {
  eventHandlerMap.delete(nodeId)
  wiredEventExports.delete(nodeId)
}

// ---------------------------------------------------------------------------
// Native event dispatch (listener events from host)
// ---------------------------------------------------------------------------

function dispatchListenerEvent(
  event: Extract<QtHostEvent, { type: "listener" }>,
): void {
  const exportId = event.listenerId
  let exportName: string | undefined
  for (const [name, id] of Object.entries(WINDOW_EVENT_EXPORTS)) {
    if (id === exportId) {
      exportName = name
      break
    }
  }
  if (exportName) {
    emitNativeEvent(event.nodeId, exportName)

    // When the native window loses hover, clear canvas hover state
    // so that hovered fragments don't stick.
    if (exportName === "onHoverLeave") {
      for (const canvasNodeId of hoveredFragments.keys()) {
        dispatchCanvasPointerMoveForHover(canvasNodeId, -1, 0, 0)
      }
    }
  }
}

// ---------------------------------------------------------------------------
// handleEvent — top-level event router
// ---------------------------------------------------------------------------

export function handleEvent(event: QtHostEvent): void {
  if (event.type === "listener") {
    if (event.traceId != null) {
      traceStack.push(event.traceId)
      traceEnterInteraction(event.traceId)
      traceRecordJs(
        event.traceId,
        "js.handle_event.enter",
        event.nodeId,
        event.listenerId,
        undefined,
        undefined,
      )
      try {
        dispatchListenerEvent(event)
        traceRecordJs(
          event.traceId,
          "js.handle_event.exit",
          event.nodeId,
          event.listenerId,
          undefined,
          undefined,
        )
      } finally {
        traceExitInteraction()
        traceStack.pop()
      }
      return
    }

    dispatchListenerEvent(event)
  } else if (event.type === "canvaspointer") {
    const { canvasNodeId, fragmentId, eventTag, x, y } = event
    dispatchCanvasPointerEvent(canvasNodeId, fragmentId, eventTag, x, y)
    if (eventTag === 3) {
      dispatchCanvasPointerMoveForHover(canvasNodeId, fragmentId, x, y)
    }
  } else if (event.type === "canvaskeyboard") {
    const { canvasNodeId, fragmentId, eventTag, qtKey, modifiers, text, repeat, nativeScanCode, nativeVirtualKey } = event
    dispatchCanvasKeyboardEvent(canvasNodeId, fragmentId, eventTag, qtKey, modifiers, text, repeat, nativeScanCode, nativeVirtualKey)
  } else if (event.type === "canvaswheel") {
    const { canvasNodeId, fragmentId, deltaX, deltaY, pixelDx, pixelDy, x, y, modifiers, phase } = event
    dispatchCanvasWheelEvent(canvasNodeId, fragmentId, deltaX, deltaY, pixelDx, pixelDy, x, y, modifiers, phase)
  } else if (event.type === "canvasmotioncomplete") {
    const { canvasNodeId, fragmentId } = event
    dispatchCanvasMotionComplete(canvasNodeId, fragmentId)
  } else if (event.type === "canvasfocuschange") {
    const { canvasNodeId, oldFragmentId, newFragmentId } = event
    dispatchCanvasFocusChange(canvasNodeId, oldFragmentId, newFragmentId)
  } else if (event.type === "canvastextinputchange") {
    const { canvasNodeId, fragmentId, text, cursor, selStart, selEnd } = event
    dispatchCanvasTextInputChange(canvasNodeId, fragmentId, text, cursor, selStart, selEnd)
  } else if (event.type === "fragmentlayout") {
    const { canvasNodeId, fragmentId, x, y, width, height } = event
    dispatchFragmentLayout(canvasNodeId, fragmentId, x, y, width, height)
  } else if (event.type === "windowfocuschange") {
    emitNativeEvent(event.nodeId, "onWindowFocusChange", { gained: event.gained })
  } else if (event.type === "windowresize") {
    emitNativeEvent(event.nodeId, "onWindowResize", { width: event.width, height: event.height })
  } else if (event.type === "windowstatechange") {
    emitNativeEvent(event.nodeId, "onWindowStateChange", { state: event.state })
  } else if (event.type === "colorschemechange") {
    colorSchemeChannel.emit(event.scheme)
  } else if (event.type === "screendpichange") {
    screenDpiChannel.emit(event.dpi)
  } else if (event.type === "filedialogresult") {
    fileDialogChannel.emit({ requestId: event.requestId, paths: event.paths })
  }
}
