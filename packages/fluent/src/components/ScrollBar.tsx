import { createSignal, createMemo, type Component } from "solid-js"
import { Show } from "solid-js"

import { useTheme } from "../theme.ts"

export interface ScrollBarProps {
  orientation?: "vertical" | "horizontal"
  contentSize: number
  viewportSize: number
  scrollOffset: number
  length?: number
  onScroll?: (offset: number) => void
}

const TRACK_THICKNESS = 6
const MIN_THUMB_SIZE = 20

export const ScrollBar: Component<ScrollBarProps> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)
  const [dragging, setDragging] = createSignal(false)
  const [dragStartPointer, setDragStartPointer] = createSignal(0)
  const [dragStartOffset, setDragStartOffset] = createSignal(0)

  const vertical = () => (props.orientation ?? "vertical") === "vertical"
  const trackLength = () => props.length ?? 200
  const maxScroll = () => Math.max(0, props.contentSize - props.viewportSize)
  const shouldShow = () => props.contentSize > props.viewportSize

  const thumbSize = createMemo(() => {
    if (props.contentSize <= 0) return 0
    return Math.max(MIN_THUMB_SIZE, (props.viewportSize / props.contentSize) * trackLength())
  })

  const thumbOffset = createMemo(() => {
    const ms = maxScroll()
    if (ms <= 0) return 0
    return (props.scrollOffset / ms) * (trackLength() - thumbSize())
  })

  const thumbFill = () => {
    const t = theme()
    if (dragging()) return t.controlPressed
    if (hovered()) return t.controlHover
    return t.controlDefault
  }

  const handleThumbPointerDown = (e: unknown) => {
    const ev = e as { x: number; y: number }
    setDragging(true)
    setDragStartPointer(vertical() ? ev.y : ev.x)
    setDragStartOffset(props.scrollOffset)
  }

  const handlePointerMove = (e: unknown) => {
    if (!dragging()) return
    const ev = e as { x: number; y: number }
    const delta = (vertical() ? ev.y : ev.x) - dragStartPointer()
    const scrollRange = trackLength() - thumbSize()
    if (scrollRange <= 0) return
    const newOffset = dragStartOffset() + (delta / scrollRange) * maxScroll()
    props.onScroll?.(Math.max(0, Math.min(maxScroll(), newOffset)))
  }

  const handlePointerUp = () => {
    setDragging(false)
  }

  return (
    <Show when={shouldShow()}>
      <group
        width={vertical() ? TRACK_THICKNESS : trackLength()}
        height={vertical() ? trackLength() : TRACK_THICKNESS}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerLeave={() => { setHovered(false); setDragging(false) }}
      >
        {/* Track */}
        <rect
          position="absolute"
          width={vertical() ? TRACK_THICKNESS : trackLength()}
          height={vertical() ? trackLength() : TRACK_THICKNESS}
          fill={theme().controlDefault}
          cornerRadius={theme().radiusSm}
        />
        {/* Thumb */}
        <rect
          position="absolute"
          x={vertical() ? 0 : thumbOffset()}
          y={vertical() ? thumbOffset() : 0}
          width={vertical() ? TRACK_THICKNESS : thumbSize()}
          height={vertical() ? thumbSize() : TRACK_THICKNESS}
          fill={thumbFill()}
          cornerRadius={theme().radiusSm}
          onPointerEnter={() => setHovered(true)}
          onPointerLeave={() => setHovered(false)}
          onPointerDown={handleThumbPointerDown}
        />
      </group>
    </Show>
  )
}
