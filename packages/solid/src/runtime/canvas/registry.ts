import { canvasFragmentStoreRemove } from "@qt-solid/core/native"

import { FragmentRendererNode } from "../fragment.ts"

// ---------------------------------------------------------------------------
// Canvas binding registry + tree lookup + event bubbling
// ---------------------------------------------------------------------------

export const CANVAS_BINDINGS = new Map<number, { root: FragmentRendererNode }>()

export const hoveredFragments = new Map<number, number>()

/** Per-canvas pointer capture: the handler-owner node that received onPointerDown. */
export const capturedPointer = new Map<number, FragmentRendererNode>()

export function registerCanvasBinding(canvasNodeId: number, root: FragmentRendererNode): void {
  CANVAS_BINDINGS.set(canvasNodeId, { root })
}

export function unregisterCanvasBinding(canvasNodeId: number): void {
  CANVAS_BINDINGS.delete(canvasNodeId)
}

export function destroyCanvasFragmentBinding(canvasNodeId: number): void {
  canvasFragmentStoreRemove(canvasNodeId)
}

export function findFragmentNode(
  root: FragmentRendererNode,
  fragmentId: number,
): FragmentRendererNode | undefined {
  if (root.fragmentId === fragmentId) return root
  let child = root.firstChild as FragmentRendererNode | null
  while (child) {
    const found = findFragmentNode(child, fragmentId)
    if (found) return found
    child = child.nextSibling as FragmentRendererNode | null
  }
  return undefined
}

export function bubbleEvent(
  start: FragmentRendererNode,
  name: string,
  payload: unknown,
): void {
  let cur: FragmentRendererNode | null = start
  while (cur) {
    const handler = cur.eventHandlers.get(name)
    if (handler) {
      handler(payload)
      return
    }
    cur = cur.parent
  }
}

/**
 * Walk from `start` up the parent chain and return the first node
 * that has a handler for `name`, or null.
 */
export function findHandlerOwner(
  start: FragmentRendererNode,
  name: string,
): FragmentRendererNode | null {
  let cur: FragmentRendererNode | null = start
  while (cur) {
    if (cur.eventHandlers.has(name)) return cur
    cur = cur.parent
  }
  return null
}

/**
 * Collect the set of ancestor nodes (inclusive) that own a handler for `name`,
 * walking from `leaf` up to root.
 */
export function collectHandlerOwners(
  leaf: FragmentRendererNode | null | undefined,
  name: string,
): Set<FragmentRendererNode> {
  const owners = new Set<FragmentRendererNode>()
  let cur = leaf
  while (cur) {
    if (cur.eventHandlers.has(name)) owners.add(cur)
    cur = cur.parent
  }
  return owners
}

/**
 * When a fragment subtree is removed, clean up hover and pointer capture
 * state so that hover/pressed don't stick.
 */
export function cleanupHoverOnRemove(
  canvasNodeId: number,
  removedRoot: FragmentRendererNode,
): void {
  const hoveredId = hoveredFragments.get(canvasNodeId)
  if (hoveredId != null && hoveredId >= 0) {
    if (isInSubtree(removedRoot, hoveredId)) {
      const hoveredNode = findFragmentNode(removedRoot, hoveredId)
      if (hoveredNode) {
        bubbleEvent(hoveredNode, "onPointerLeave", { x: 0, y: 0 })
      }
      hoveredFragments.set(canvasNodeId, -1)
    }
  }

  // Clear pointer capture if the captured node is inside the removed subtree
  const captured = capturedPointer.get(canvasNodeId)
  if (captured && isInSubtree(removedRoot, captured.fragmentId)) {
    capturedPointer.delete(canvasNodeId)
  }
}

function isInSubtree(root: FragmentRendererNode, fragmentId: number): boolean {
  return findFragmentNode(root, fragmentId) != null
}
