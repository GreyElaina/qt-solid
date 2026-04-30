import {
  canvasFragmentCreate,
  canvasFragmentDestroy,
  canvasFragmentDetachChild,
  canvasFragmentInsertChild,
  canvasFragmentSetProp,
  canvasFragmentSetF64Prop,
  canvasFragmentSetStringProp,
  canvasFragmentSetBoolProp,
  canvasFragmentSetEncodedImage,
  canvasFragmentClearImage,
  canvasFragmentSetMotionTarget,
  canvasFragmentGetWorldBounds,
  canvasFragmentSetLayoutFlip,
  canvasFragmentStoreEnsure,
} from "@qt-solid/core/native"

import type { QtMotionConfig } from "../qt-intrinsics.ts"
import type { TransitionSpec } from "../app/motion/types.ts"
import { setLayoutId as registrySetLayoutId, unsetLayoutId as registryUnsetLayoutId } from "../app/motion/layout-id.ts"
import type { QtRendererNode } from "./renderer.ts"

// ---------------------------------------------------------------------------
// Fragment renderer node — JS-side linked list for canvas fragments
// ---------------------------------------------------------------------------

export const FRAGMENT_ROOT_ID = -1

export class FragmentRendererNode implements QtRendererNode {
  readonly nodeKind = "fragment" as const
  readonly canvasNodeId: number
  readonly fragmentId: number
  readonly kind: string
  readonly eventHandlers: Map<string, (...args: unknown[]) => void> = new Map()
  _motionCompleteCallback: (() => void) | null = null

  parent: FragmentRendererNode | null = null
  firstChild: FragmentRendererNode | null = null
  nextSibling: FragmentRendererNode | null = null
  previousSibling: FragmentRendererNode | null = null
  lastChild: FragmentRendererNode | null = null

  constructor(canvasNodeId: number, fragmentId: number, kind: string) {
    this.canvasNodeId = canvasNodeId
    this.fragmentId = fragmentId
    this.kind = kind
  }

  get id(): number {
    return this.fragmentId
  }

  isTextNode(): boolean {
    return false
  }

  insertChild(child: QtRendererNode, anchor?: QtRendererNode | null): void {
    const fragmentChild = child as FragmentRendererNode
    const fragmentAnchor = anchor as FragmentRendererNode | null | undefined

    if (fragmentChild.parent) {
      fragmentChild.parent.removeChild(fragmentChild)
    }

    fragmentChild.parent = this

    if (fragmentAnchor) {
      fragmentChild.nextSibling = fragmentAnchor
      fragmentChild.previousSibling = fragmentAnchor.previousSibling
      if (fragmentAnchor.previousSibling) {
        fragmentAnchor.previousSibling.nextSibling = fragmentChild
      } else {
        this.firstChild = fragmentChild
      }
      fragmentAnchor.previousSibling = fragmentChild
    } else {
      fragmentChild.previousSibling = this.lastChild
      fragmentChild.nextSibling = null
      if (this.lastChild) {
        this.lastChild.nextSibling = fragmentChild
      } else {
        this.firstChild = fragmentChild
      }
      this.lastChild = fragmentChild
    }

    canvasFragmentInsertChild(
      this.canvasNodeId,
      this.fragmentId,
      fragmentChild.fragmentId,
      fragmentAnchor?.fragmentId ?? null,
    )
  }

  removeChild(child: QtRendererNode): void {
    const fragmentChild = child as FragmentRendererNode

    if (fragmentChild.previousSibling) {
      fragmentChild.previousSibling.nextSibling = fragmentChild.nextSibling
    } else {
      this.firstChild = fragmentChild.nextSibling
    }
    if (fragmentChild.nextSibling) {
      fragmentChild.nextSibling.previousSibling = fragmentChild.previousSibling
    } else {
      this.lastChild = fragmentChild.previousSibling
    }

    fragmentChild.parent = null
    fragmentChild.previousSibling = null
    fragmentChild.nextSibling = null

    canvasFragmentDetachChild(
      this.canvasNodeId,
      this.fragmentId,
      fragmentChild.fragmentId,
    )
  }

  destroy(): void {
    if (this.fragmentId !== FRAGMENT_ROOT_ID) {
      canvasFragmentDestroy(this.canvasNodeId, this.fragmentId)
    }
  }

  // Motion interface
  setMotionTarget(
    target: import("@qt-solid/core/native").QtMotionTarget,
    transition: import("@qt-solid/core/native").QtPerPropertyTransition,
    delay?: number | null,
  ): void {
    const animating = canvasFragmentSetMotionTarget(
      this.canvasNodeId,
      this.fragmentId,
      target,
      transition,
      delay,
    )
    if (!animating) {
      const cb = this._motionCompleteCallback
      if (cb) {
        this._motionCompleteCallback = null
        cb()
      }
    }
  }

  setMotionConfig(_config: QtMotionConfig): void {
    // Fragment motion does not use compositor layer config — no-op.
  }

  onMotionComplete(callback: () => void): void {
    this._motionCompleteCallback = callback
  }

  setLayoutId(layoutId: string, transition?: TransitionSpec): void {
    registrySetLayoutId(
      { canvasNodeId: this.canvasNodeId, fragmentId: this.fragmentId },
      layoutId,
      transition,
    )
  }

  unsetLayoutId(layoutId: string): void {
    registryUnsetLayoutId(
      { canvasNodeId: this.canvasNodeId, fragmentId: this.fragmentId },
      layoutId,
    )
  }
}

// ---------------------------------------------------------------------------
// Fragment prop writing
// ---------------------------------------------------------------------------

function lowerObjectProp(key: string, value: Record<string, unknown>): unknown {
  if (
    key === "cornerRadius" &&
    "topLeft" in value &&
    "topRight" in value &&
    "bottomRight" in value &&
    "bottomLeft" in value
  ) {
    return {
      type: "radii",
      topLeft: value.topLeft as number,
      topRight: value.topRight as number,
      bottomRight: value.bottomRight as number,
      bottomLeft: value.bottomLeft as number,
    }
  }

  // Gradient brushes — convert from user-facing shape to napi wire format
  if ("type" in value && "stops" in value) {
    const stops = value.stops as Array<{ offset: number; color: string }>
    const stopOffsets = stops.map((s) => s.offset)
    const stopColors = stops.map((s) => s.color)

    switch (value.type) {
      case "linearGradient":
        return {
          type: "lineargradient",
          startX: value.startX as number,
          startY: value.startY as number,
          endX: value.endX as number,
          endY: value.endY as number,
          stopOffsets,
          stopColors,
        }
      case "radialGradient":
        return {
          type: "radialgradient",
          centerX: value.centerX as number,
          centerY: value.centerY as number,
          radius: value.radius as number,
          stopOffsets,
          stopColors,
        }
      case "sweepGradient":
        return {
          type: "sweepgradient",
          centerX: value.centerX as number,
          centerY: value.centerY as number,
          startAngle: value.startAngle as number,
          endAngle: value.endAngle as number,
          stopOffsets,
          stopColors,
        }
    }
  }

  // Box shadow — add discriminant type and pass through inset
  if (
    key === "shadow" &&
    "offsetX" in value &&
    "offsetY" in value &&
    "blur" in value &&
    "color" in value
  ) {
    return {
      type: "boxshadow",
      offsetX: value.offsetX as number,
      offsetY: value.offsetY as number,
      blur: value.blur as number,
      color: value.color as string,
      inset: (value.inset as boolean) ?? false,
    }
  }

  // Per-side border — { width, color } → Border wire variant
  if (
    (key === "borderTop" || key === "borderRight" || key === "borderBottom" || key === "borderLeft") &&
    "width" in value &&
    "color" in value
  ) {
    return {
      type: "border",
      width: value.width as number,
      color: value.color as string,
    }
  }

  // Grid template tracks — convert array of track sizes to napi wire format
  if (
    (key === "gridTemplateRows" || key === "gridTemplateColumns") &&
    Array.isArray(value)
  ) {
    return {
      type: "gridtracks",
      tracks: (value as Array<number | string>).map((t) =>
        typeof t === "number" ? String(t) : t,
      ),
    }
  }

  return value
}

export function writeFragmentProp(
  canvasNodeId: number,
  fragmentId: number,
  key: string,
  value: unknown,
): void {
  if (value == null) return
  if (typeof value === "number") {
    canvasFragmentSetF64Prop(canvasNodeId, fragmentId, key, value)
  } else if (typeof value === "boolean") {
    canvasFragmentSetBoolProp(canvasNodeId, fragmentId, key, value)
  } else if (typeof value === "string") {
    if (key === "blendMode") {
      canvasFragmentSetProp(canvasNodeId, fragmentId, key, { type: "blendmode", value } as never)
    } else {
      canvasFragmentSetStringProp(canvasNodeId, fragmentId, key, value)
    }
  } else if (typeof value === "object") {
    const wrapped = lowerObjectProp(key, value as Record<string, unknown>)
    canvasFragmentSetProp(canvasNodeId, fragmentId, key, wrapped as never)
  }
}

// ---------------------------------------------------------------------------
// Canvas fragment binding factory
// ---------------------------------------------------------------------------

export function createCanvasFragmentBinding(canvasNodeId: number): {
  root: FragmentRendererNode
} {
  canvasFragmentStoreEnsure(canvasNodeId)
  const root = new FragmentRendererNode(canvasNodeId, FRAGMENT_ROOT_ID, "root")
  return { root }
}
