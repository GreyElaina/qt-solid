import { createSignal, type Component } from "solid-js"
import { canvasFragmentGetWorldBounds } from "@qt-solid/core/native"
import type { CanvasNodeHandle } from "@qt-solid/solid"

import { useTheme } from "../theme.ts"

export interface SliderProps {
  value?: number
  min?: number
  max?: number
  step?: number
  disabled?: boolean
  width?: number
  onChange?: (value: number) => void
}

const TRACK_H = 4
const HANDLE_SIZE = 22
const INNER_R_NORMAL = 5
const INNER_R_HOVER = 6.5
const INNER_R_PRESSED = 4

export const Slider: Component<SliderProps> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)
  const [pressed, setPressed] = createSignal(false)
  const [dragging, setDragging] = createSignal(false)
  let sliderRef: CanvasNodeHandle | undefined

  const min = () => props.min ?? 0
  const max = () => props.max ?? 100
  const step = () => props.step ?? 1
  const value = () => props.value ?? min()
  const trackWidth = () => props.width ?? 200

  const ratio = () => {
    const range = max() - min()
    if (range <= 0) return 0
    return (value() - min()) / range
  }

  const handleX = () => {
    const usable = trackWidth() - HANDLE_SIZE
    return ratio() * usable + HANDLE_SIZE / 2
  }

  const innerR = () => {
    if (props.disabled) return INNER_R_NORMAL
    if (pressed() || dragging()) return INNER_R_PRESSED
    if (hovered()) return INNER_R_HOVER
    return INNER_R_NORMAL
  }

  const filledWidth = () => ratio() * trackWidth()

  const trackY = () => (HANDLE_SIZE - TRACK_H) / 2

  const computeValue = (pointerX: number) => {
    let localX = pointerX
    if (sliderRef) {
      const bounds = canvasFragmentGetWorldBounds(sliderRef.canvasNodeId, sliderRef.fragmentId)
      if (bounds) localX = pointerX - bounds.x
    }
    const clamped = Math.max(0, Math.min(localX, trackWidth()))
    const raw = min() + (clamped / trackWidth()) * (max() - min())
    const s = step()
    const snapped = Math.round(raw / s) * s
    return Math.max(min(), Math.min(max(), snapped))
  }

  const handlePointerDown = (e: unknown) => {
    if (props.disabled) return
    setDragging(true)
    setPressed(true)
    const x = (e as { x: number }).x
    props.onChange?.(computeValue(x))
  }

  const handlePointerMove = (e: unknown) => {
    if (!dragging() || props.disabled) return
    const x = (e as { x: number }).x
    props.onChange?.(computeValue(x))
  }

  const handlePointerUp = () => {
    setDragging(false)
    setPressed(false)
  }

  const accentColor = () => {
    const t = theme()
    if (props.disabled) return t.accentDisabled
    if (pressed() || dragging()) return t.accentPressed
    if (hovered()) return t.accentHover
    return t.accentDefault
  }

  const handleOuterFill = () => {
    const t = theme()
    return t.controlDefault
  }

  return (
    <group
      ref={(node: CanvasNodeHandle) => { sliderRef = node }}
      width={trackWidth()}
      height={HANDLE_SIZE}
      focusable={!props.disabled}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => { setHovered(false); setPressed(false); setDragging(false) }}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
    >
      {/* Track background */}
      <rect
        position="absolute"
        x={0}
        y={trackY()}
        width={trackWidth()}
        height={TRACK_H}
        fill={theme().strokeDefault}
        cornerRadius={2}
      />
      {/* Filled track */}
      <rect
        position="absolute"
        x={0}
        y={trackY()}
        width={filledWidth()}
        height={TRACK_H}
        fill={accentColor()}
        cornerRadius={2}
      />
      {/* Handle outer circle */}
      <circle
        position="absolute"
        cx={handleX()}
        cy={HANDLE_SIZE / 2}
        r={HANDLE_SIZE / 2}
        fill={handleOuterFill()}
        stroke={theme().strokeDefault}
        strokeWidth={1}
      />
      {/* Handle inner circle */}
      <circle
        position="absolute"
        cx={handleX()}
        cy={HANDLE_SIZE / 2}
        r={innerR()}
        fill={accentColor()}
      />
    </group>
  )
}
