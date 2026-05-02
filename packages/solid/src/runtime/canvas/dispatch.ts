import { FragmentRendererNode } from "../fragment.ts"
import { buildCanvasKeyboardPayload } from "../key-mapping.ts"
import type { WheelEventPayload } from "../../qt-intrinsics.ts"
import {
  CANVAS_BINDINGS,
  hoveredFragments,
  capturedPointer,
  findFragmentNode,
  findHandlerOwner,
  collectHandlerOwners,
  bubbleEvent,
} from "./registry.ts"

// ---------------------------------------------------------------------------
// Handled canvas event names
// ---------------------------------------------------------------------------

export const HANDLED_EVENT_NAMES = new Set([
  "onClick", "onDoubleClick", "onPointerDown", "onPointerUp", "onPointerMove",
  "onPointerEnter", "onPointerLeave",
  "onKeyDown", "onKeyUp",
  "onWheel",
  "onFocusIn", "onFocusOut",
  "onTextChange",
  "onLayout",
])

// Per-canvas flag: suppress the onClick that follows a double-click's trailing release.
const suppressNextClick = new Set<number>()

// ---------------------------------------------------------------------------
// Canvas event dispatch functions
// ---------------------------------------------------------------------------

export function dispatchCanvasPointerEvent(
  canvasNodeId: number,
  fragmentId: number,
  eventTag: number,
  x: number,
  y: number,
): void {
  const binding = CANVAS_BINDINGS.get(canvasNodeId)
  if (!binding) {
    suppressNextClick.delete(canvasNodeId)
    capturedPointer.delete(canvasNodeId)
    return
  }

  let skipClick = false

  if (eventTag === 1) {
    suppressNextClick.delete(canvasNodeId)
  } else if (eventTag === 2) {
    skipClick = suppressNextClick.delete(canvasNodeId)
  }

  const payload = { x, y }

  switch (eventTag) {
    case 1: {
      // Pointer down: hit-test node, find handler owner, capture it
      const node = findFragmentNode(binding.root, fragmentId)
      if (!node) return
      const owner = findHandlerOwner(node, "onPointerDown")
      if (owner) {
        capturedPointer.set(canvasNodeId, owner)
      }
      bubbleEvent(node, "onPointerDown", payload)
      break
    }
    case 2: {
      // Pointer up: route to captured node if available, else hit-test
      const captured = capturedPointer.get(canvasNodeId)
      capturedPointer.delete(canvasNodeId)
      if (captured) {
        bubbleEvent(captured, "onPointerUp", payload)
        if (!skipClick) {
          bubbleEvent(captured, "onClick", payload)
        }
      } else {
        const node = findFragmentNode(binding.root, fragmentId)
        if (!node) return
        bubbleEvent(node, "onPointerUp", payload)
        if (!skipClick) {
          bubbleEvent(node, "onClick", payload)
        }
      }
      break
    }
    case 3: {
      // Pointer move: route to captured node if available (drag), else hit-test
      const captured = capturedPointer.get(canvasNodeId)
      if (captured) {
        bubbleEvent(captured, "onPointerMove", payload)
      } else {
        const node = findFragmentNode(binding.root, fragmentId)
        if (node) bubbleEvent(node, "onPointerMove", payload)
      }
      break
    }
    case 5: {
      // Double click
      const node = findFragmentNode(binding.root, fragmentId)
      if (!node) return
      suppressNextClick.add(canvasNodeId)
      bubbleEvent(node, "onDoubleClick", payload)
      break
    }
  }
}

export function dispatchCanvasContextMenuEvent(
  canvasNodeId: number,
  fragmentId: number,
  x: number,
  y: number,
  screenX: number,
  screenY: number,
): void {
  const binding = CANVAS_BINDINGS.get(canvasNodeId)
  if (!binding) return

  const node = findFragmentNode(binding.root, fragmentId)
  if (!node) return
  bubbleEvent(node, "onContextMenu", { x, y, screenX, screenY })
}

// ---------------------------------------------------------------------------
// Hover dispatch — ancestor-diff based enter/leave
// ---------------------------------------------------------------------------

/**
 * Hover tracking uses ancestor-diff: instead of comparing raw leaf fragment
 * ids, we compare the set of ancestor nodes that own onPointerEnter/Leave
 * handlers. This prevents false leave/enter when moving between child
 * fragments of the same logical control.
 */
export function dispatchCanvasPointerMoveForHover(
  canvasNodeId: number,
  fragmentId: number,
  x: number,
  y: number,
): void {
  const binding = CANVAS_BINDINGS.get(canvasNodeId)
  if (!binding) return

  const prevLeafId = hoveredFragments.get(canvasNodeId) ?? -1
  if (prevLeafId === fragmentId) return

  hoveredFragments.set(canvasNodeId, fragmentId)

  const prevLeaf = prevLeafId >= 0 ? findFragmentNode(binding.root, prevLeafId) : null
  const nextLeaf = fragmentId >= 0 ? findFragmentNode(binding.root, fragmentId) : null

  // Collect handler owners for enter and leave independently
  const prevEnterOwners = collectHandlerOwners(prevLeaf, "onPointerEnter")
  const prevLeaveOwners = collectHandlerOwners(prevLeaf, "onPointerLeave")
  const nextEnterOwners = collectHandlerOwners(nextLeaf, "onPointerEnter")
  const nextLeaveOwners = collectHandlerOwners(nextLeaf, "onPointerLeave")

  const payload = { x, y }

  // Fire leave on owners that were hovered but are no longer
  // (had a leave handler and were in the prev owner set for enter)
  for (const owner of prevLeaveOwners) {
    if (!nextLeaveOwners.has(owner)) {
      const handler = owner.eventHandlers.get("onPointerLeave")
      if (handler) handler(payload)
    }
  }

  // Fire enter on owners that are newly hovered
  for (const owner of nextEnterOwners) {
    if (!prevEnterOwners.has(owner)) {
      const handler = owner.eventHandlers.get("onPointerEnter")
      if (handler) handler(payload)
    }
  }
}

export function dispatchCanvasMotionComplete(
  canvasNodeId: number,
  fragmentId: number,
): void {
  const binding = CANVAS_BINDINGS.get(canvasNodeId)
  if (!binding) return
  const node = findFragmentNode(binding.root, fragmentId)
  if (!node) return
  const cb = node._motionCompleteCallback
  if (cb) {
    node._motionCompleteCallback = null
    cb()
  }
}

export function dispatchCanvasFocusChange(
  canvasNodeId: number,
  oldFragmentId: number,
  newFragmentId: number,
): void {
  const binding = CANVAS_BINDINGS.get(canvasNodeId)
  if (!binding) return

  if (oldFragmentId >= 0) {
    const oldNode = findFragmentNode(binding.root, oldFragmentId)
    oldNode?.eventHandlers.get("onFocusOut")?.({})
  }

  if (newFragmentId >= 0) {
    const newNode = findFragmentNode(binding.root, newFragmentId)
    newNode?.eventHandlers.get("onFocusIn")?.({})
  }
}

export function dispatchCanvasKeyboardEvent(
  canvasNodeId: number,
  fragmentId: number,
  eventTag: number,
  qtKey: number,
  modifiers: number,
  text: string,
  repeat: boolean,
  nativeScanCode: number,
  nativeVirtualKey: number,
): void {
  const binding = CANVAS_BINDINGS.get(canvasNodeId)
  if (!binding) return

  const node = fragmentId >= 0 ? findFragmentNode(binding.root, fragmentId) : null
  if (!node) return

  const payload = buildCanvasKeyboardPayload(
    qtKey, modifiers, text, repeat, nativeScanCode, nativeVirtualKey,
  )

  switch (eventTag) {
    case 1:
      bubbleEvent(node, "onKeyDown", payload)
      break
    case 2:
      bubbleEvent(node, "onKeyUp", payload)
      break
  }
}

export function dispatchCanvasTextInputChange(
  canvasNodeId: number,
  fragmentId: number,
  text: string,
  cursor: number,
  selStart: number,
  selEnd: number,
): void {
  const binding = CANVAS_BINDINGS.get(canvasNodeId)
  if (!binding) return
  const node = fragmentId >= 0 ? findFragmentNode(binding.root, fragmentId) : null
  if (!node) return
  const handler = node.eventHandlers.get("onTextChange")
  if (handler) handler({ text, cursor, selStart, selEnd })
}

export function dispatchCanvasWheelEvent(
  canvasNodeId: number,
  fragmentId: number,
  deltaX: number,
  deltaY: number,
  pixelDeltaX: number,
  pixelDeltaY: number,
  x: number,
  y: number,
  modifiers: number,
  phase: number,
): void {
  const binding = CANVAS_BINDINGS.get(canvasNodeId)
  if (!binding) return

  const node = fragmentId >= 0 ? findFragmentNode(binding.root, fragmentId) : null
  if (!node) return

  // Qt deltas are content-on-screen movement; scroll offsets are the inverse.
  // Negate so that deltaY>0 means "increase scrollY / reveal content below".
  // Per-axis: prefer pixelDelta (trackpad), fall back to angleDelta (mouse wheel)
  // normalized from 1/8-degree units to ~40px per notch.
  const WHEEL_STEP_PX = 40
  const rawX = pixelDeltaX !== 0 ? pixelDeltaX : (deltaX / 120) * WHEEL_STEP_PX
  const rawY = pixelDeltaY !== 0 ? pixelDeltaY : (deltaY / 120) * WHEEL_STEP_PX

  const payload: WheelEventPayload = {
    deltaX: -rawX,
    deltaY: -rawY,
    angleDeltaX: deltaX,
    angleDeltaY: deltaY,
    pixelDeltaX,
    pixelDeltaY,
    x,
    y,
    phase,
    ctrlKey: (modifiers & 0x04000000) !== 0,
    shiftKey: (modifiers & 0x02000000) !== 0,
    altKey: (modifiers & 0x08000000) !== 0,
    metaKey: (modifiers & 0x10000000) !== 0,
  }

  bubbleEvent(node, "onWheel", payload)
}

export function dispatchFragmentLayout(
  canvasNodeId: number,
  fragmentId: number,
  x: number,
  y: number,
  width: number,
  height: number,
): void {
  const binding = CANVAS_BINDINGS.get(canvasNodeId)
  if (!binding) return
  const node = findFragmentNode(binding.root, fragmentId)
  if (!node) return
  const handler = node.eventHandlers.get("onLayout")
  if (handler) handler({ x, y, width, height })
}
