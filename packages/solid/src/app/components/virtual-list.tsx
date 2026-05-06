import {
  createSignal,
  createEffect,
  createMemo,
  For,
  type Component,
  type JSX,
} from "solid-js"

import {
  canvasFragmentSetScrollOffset,
  canvasFragmentGetContentSize,
  canvasFragmentGetWorldBounds,
  canvasFragmentRequestRepaint,
} from "@qt-solid/core/native"

import type { SizingValue, WheelEventPayload } from "../../intrinsics.ts"
import type { FragmentRendererNode } from "../../runtime/fragment.ts"

export interface VirtualListProps {
  /** Total number of items in the list. */
  itemCount: number
  /** Fixed height of each item in logical pixels. */
  itemHeight: number
  /** Render function — receives item index, returns JSX. */
  renderItem: (index: number) => JSX.Element
  /** Number of extra items to render above/below viewport. Default 2. */
  overscan?: number
  /** Container width (explicit or flex-driven). */
  w?: SizingValue
  /** Container height (explicit or flex-driven). */
  h?: SizingValue
}

export const VirtualList: Component<VirtualListProps> = (props) => {
  let containerRef: FragmentRendererNode | undefined

  const [scrollY, setScrollY] = createSignal(0)
  const [viewportHeight, setViewportHeight] = createSignal(0)

  const overscan = () => props.overscan ?? 2
  const totalHeight = () => props.itemCount * props.itemHeight

  // Derive visible index range from scroll offset + viewport
  const visibleRange = createMemo(() => {
    const vp = viewportHeight()
    if (vp <= 0) return { start: 0, end: 0 }

    const offset = scrollY()
    const rawStart = Math.floor(offset / props.itemHeight) - overscan()
    const rawEnd = Math.ceil((offset + vp) / props.itemHeight) + overscan()

    const start = Math.max(0, rawStart)
    const end = Math.min(props.itemCount, rawEnd)
    return { start, end }
  })

  // Array of visible indices — drives <For> reconciliation
  const visibleIndices = createMemo(() => {
    const { start, end } = visibleRange()
    const indices: number[] = []
    for (let i = start; i < end; i++) {
      indices.push(i)
    }
    return indices
  })

  // Sync scroll offset to native fragment
  createEffect(() => {
    const y = scrollY()
    if (!containerRef) return
    canvasFragmentSetScrollOffset(
      containerRef.canvasNodeId,
      containerRef.fragmentId,
      0,
      y,
    )
    canvasFragmentRequestRepaint(containerRef.canvasNodeId)
  })

  const clampScroll = (value: number): number => {
    const maxScroll = Math.max(0, totalHeight() - viewportHeight())
    return Math.max(0, Math.min(value, maxScroll))
  }

  const onWheel = (e: WheelEventPayload) => {
    setScrollY((prev) => clampScroll(prev + e.deltaY))
  }

  const onLayout = (e: { x: number; y: number; width: number; height: number }) => {
    setViewportHeight(e.height)
  }

  return (
    <rect
      ref={(node: FragmentRendererNode) => { containerRef = node }}
      w={props.w}
      h={props.h}
      overflowY="scroll"
      overflowX="clip"
      clip={true}
      onWheel={onWheel}
      onLayout={onLayout}
    >
      {/* Spacer — establishes total scrollable content height */}
      <rect h={totalHeight()} w={0} absolute />
      {/* Visible items */}
      <For each={visibleIndices()}>
        {(index) => (
          <rect
            absolute
            transformY={index * props.itemHeight}
            h={props.itemHeight}
            w="fill"
          >
            {props.renderItem(index)}
          </rect>
        )}
      </For>
    </rect>
  )
}
