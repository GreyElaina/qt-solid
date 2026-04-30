import { createSignal, onCleanup, type Component, type JSX } from "solid-js"

import {
  canvasFragmentScrollDrive,
  canvasFragmentScrollRelease,
  canvasFragmentGetContentSize,
  canvasFragmentGetWorldBounds,
} from "@qt-solid/core/native"

import type { WheelEventPayload, CanvasNodeHandle } from "../../qt-intrinsics.ts"

export interface ScrollViewProps {
  children?: JSX.Element
  width?: number
  height?: number
  flexGrow?: number
  flexShrink?: number
  direction?: "vertical" | "horizontal" | "both"
  /** Spring stiffness for momentum deceleration (default: 170) */
  springStiffness?: number
  /** Spring damping for momentum deceleration (default: 26) */
  springDamping?: number
}

export const ScrollView: Component<ScrollViewProps> = (props) => {
  let containerRef: CanvasNodeHandle | undefined

  // Track current accumulated scroll offset (driven value)
  const [scrollX, setScrollX] = createSignal(0)
  const [scrollY, setScrollY] = createSignal(0)

  // Idle timeout for discrete wheel (phase=0) release
  let idleTimer: ReturnType<typeof setTimeout> | undefined

  const direction = () => props.direction ?? "vertical"

  const getViewportSize = (axis: "x" | "y"): number => {
    if (!containerRef) return 0
    if (axis === "x" && props.width != null) return props.width
    if (axis === "y" && props.height != null) return props.height
    const bounds = canvasFragmentGetWorldBounds(
      containerRef.canvasNodeId,
      containerRef.fragmentId,
    )
    if (!bounds) return 0
    return axis === "x" ? bounds.width : bounds.height
  }

  const getMaxScroll = (axis: "x" | "y"): number => {
    if (!containerRef) return 0
    const content = canvasFragmentGetContentSize(
      containerRef.canvasNodeId,
      containerRef.fragmentId,
    )
    const viewport = getViewportSize(axis)
    if (!content || viewport <= 0) return 0
    const contentSize = axis === "x" ? content.width : content.height
    return Math.max(0, contentSize - viewport)
  }

  const clampScroll = (value: number, axis: "x" | "y"): number => {
    return Math.max(0, Math.min(value, getMaxScroll(axis)))
  }

  const releaseScroll = () => {
    if (!containerRef) return
    const clampedX = clampScroll(scrollX(), "x")
    const clampedY = clampScroll(scrollY(), "y")
    canvasFragmentScrollRelease(
      containerRef.canvasNodeId,
      containerRef.fragmentId,
      clampedX,
      clampedY,
      props.springStiffness,
      props.springDamping,
    )
  }

  const driveScroll = (x: number, y: number) => {
    if (!containerRef) return
    setScrollX(x)
    setScrollY(y)
    canvasFragmentScrollDrive(
      containerRef.canvasNodeId,
      containerRef.fragmentId,
      x,
      y,
    )
  }

  const onWheel = (e: WheelEventPayload) => {
    const dir = direction()

    let newX = scrollX()
    let newY = scrollY()

    if (dir === "vertical" || dir === "both") {
      newY += e.deltaY
    }
    if (dir === "horizontal" || dir === "both") {
      newX += e.deltaX
    }

    // Drive without clamping (allows overscroll for rubber-band)
    driveScroll(newX, newY)

    // Handle release based on phase
    if (e.phase === 3) {
      // Phase 3 = gesture end — release immediately
      if (idleTimer != null) {
        clearTimeout(idleTimer)
        idleTimer = undefined
      }
      releaseScroll()
    } else if (e.phase === 0) {
      // Discrete wheel (no phase info) — use idle timeout
      if (idleTimer != null) clearTimeout(idleTimer)
      idleTimer = setTimeout(() => {
        idleTimer = undefined
        releaseScroll()
      }, 150)
    }
    // Phase 1 (begin), 2 (update) — keep driving, don't release
    // Phase 4 (momentum) — OS-level momentum, we let our spring handle it after phase 3
  }

  onCleanup(() => {
    if (idleTimer != null) clearTimeout(idleTimer)
  })

  const overflowX = () => {
    const dir = direction()
    return dir === "horizontal" || dir === "both" ? "scroll" : "clip"
  }

  const overflowY = () => {
    const dir = direction()
    return dir === "vertical" || dir === "both" ? "scroll" : "clip"
  }

  return (
    <rect
      ref={(node: CanvasNodeHandle) => { containerRef = node }}
      width={props.width}
      height={props.height}
      flexGrow={props.flexGrow}
      flexShrink={props.flexShrink}
      overflowX={overflowX()}
      overflowY={overflowY()}
      clip={true}
      onWheel={onWheel}
    >
      {props.children}
    </rect>
  )
}
