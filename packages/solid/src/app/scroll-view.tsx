import { createSignal, createEffect, type Component, type JSX } from "solid-js"

import {
  canvasFragmentSetScrollOffset,
  canvasFragmentGetContentSize,
  canvasFragmentGetWorldBounds,
  canvasFragmentRequestRepaint,
} from "@qt-solid/core/native"

import type { WheelEventPayload, CanvasNodeHandle } from "../qt-intrinsics.ts"

export interface ScrollViewProps {
  children?: JSX.Element
  width?: number
  height?: number
  flexGrow?: number
  flexShrink?: number
  direction?: "vertical" | "horizontal" | "both"
}

export const ScrollView: Component<ScrollViewProps> = (props) => {
  let containerRef: CanvasNodeHandle | undefined

  const [scrollX, setScrollX] = createSignal(0)
  const [scrollY, setScrollY] = createSignal(0)

  const direction = () => props.direction ?? "vertical"

  const getViewportSize = (axis: "x" | "y"): number => {
    if (!containerRef) return 0
    // Prefer explicit prop; fall back to actual layout bounds from native
    if (axis === "x" && props.width != null) return props.width
    if (axis === "y" && props.height != null) return props.height
    const bounds = canvasFragmentGetWorldBounds(
      containerRef.canvasNodeId,
      containerRef.fragmentId,
    )
    if (!bounds) return 0
    return axis === "x" ? bounds.width : bounds.height
  }

  const clamp = (value: number, axis: "x" | "y"): number => {
    if (!containerRef) return 0
    const content = canvasFragmentGetContentSize(
      containerRef.canvasNodeId,
      containerRef.fragmentId,
    )
    const viewport = getViewportSize(axis)
    if (!content || viewport <= 0) return 0
    const contentSize = axis === "x" ? content.width : content.height
    const maxScroll = Math.max(0, contentSize - viewport)
    return Math.max(0, Math.min(value, maxScroll))
  }

  createEffect(() => {
    const x = scrollX()
    const y = scrollY()
    if (!containerRef) return
    canvasFragmentSetScrollOffset(
      containerRef.canvasNodeId,
      containerRef.fragmentId,
      x,
      y,
    )
    canvasFragmentRequestRepaint(containerRef.canvasNodeId)
  })

  const onWheel = (e: WheelEventPayload) => {
    const dir = direction()
    if (dir === "vertical" || dir === "both") {
      setScrollY((prev) => clamp(prev + e.deltaY, "y"))
    }
    if (dir === "horizontal" || dir === "both") {
      setScrollX((prev) => clamp(prev + e.deltaX, "x"))
    }
  }

  return (
    <rect
      ref={(node: CanvasNodeHandle) => { containerRef = node }}
      width={props.width}
      height={props.height}
      flexGrow={props.flexGrow}
      flexShrink={props.flexShrink}
      overflow="scroll"
      clip={true}
      onWheel={onWheel}
    >
      {props.children}
    </rect>
  )
}
