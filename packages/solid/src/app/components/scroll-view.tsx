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
  /** Spring stiffness for overscroll return-to-bounds (default: 170) */
  springStiffness?: number
  /** Spring damping for overscroll return-to-bounds (default: 26) */
  springDamping?: number
}

export const ScrollView: Component<ScrollViewProps> = (props) => {
  let containerRef: CanvasNodeHandle | undefined

  // Current accumulated scroll offset (driven value, may exceed bounds)
  const [scrollX, setScrollX] = createSignal(0)
  const [scrollY, setScrollY] = createSignal(0)

  // Idle timer for release after discrete wheel or momentum end
  let idleTimer: ReturnType<typeof setTimeout> | undefined
  // Whether we are in the middle of a trackpad gesture (phase 1..4)
  let gestureActive = false

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

  const releaseToClamp = () => {
    if (!containerRef) return
    const x = scrollX()
    const y = scrollY()
    const clampedX = clampScroll(x, "x")
    const clampedY = clampScroll(y, "y")

    // Sync JS state to the target so the next gesture starts from the right place.
    setScrollX(clampedX)
    setScrollY(clampedY)

    // Only animate if actually out of bounds.
    if (Math.abs(x - clampedX) > 0.5 || Math.abs(y - clampedY) > 0.5) {
      canvasFragmentScrollRelease(
        containerRef.canvasNodeId,
        containerRef.fragmentId,
        clampedX,
        clampedY,
        props.springStiffness,
        props.springDamping,
      )
    }
  }

  const scheduleIdleRelease = () => {
    if (idleTimer != null) clearTimeout(idleTimer)
    idleTimer = setTimeout(() => {
      idleTimer = undefined
      gestureActive = false
      releaseToClamp()
    }, 120)
  }

  const driveScroll = (x: number, y: number) => {
    if (!containerRef) return
    // Clamp to bounds — no rubber-band for now.
    const cx = clampScroll(x, "x")
    const cy = clampScroll(y, "y")
    setScrollX(cx)
    setScrollY(cy)
    canvasFragmentScrollDrive(
      containerRef.canvasNodeId,
      containerRef.fragmentId,
      cx,
      cy,
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

    driveScroll(newX, newY)

    if (e.phase === 1) {
      // Trackpad begin — clear any pending release, mark gesture active
      gestureActive = true
      if (idleTimer != null) {
        clearTimeout(idleTimer)
        idleTimer = undefined
      }
    } else if (e.phase === 2) {
      // Trackpad update — finger is on pad, keep driving
    } else if (e.phase === 3) {
      // Trackpad end — finger lifted. If OS sends momentum (phase 4) it will
      // arrive shortly; schedule a deferred release that momentum events cancel.
      scheduleIdleRelease()
    } else if (e.phase === 4) {
      // OS momentum — apply the delta (already done above) and reschedule idle.
      // The OS provides decelerated deltas; we just follow them.
      scheduleIdleRelease()
    } else {
      // Discrete mouse wheel (phase=0) — clamp after idle
      scheduleIdleRelease()
    }
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
