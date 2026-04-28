import { createSignal, type Component } from "solid-js"

import { useTheme } from "../theme.ts"
import { LineEdit } from "./LineEdit.tsx"

export interface SpinBoxProps {
  value?: number
  min?: number
  max?: number
  step?: number
  disabled?: boolean
  width?: number
  onChange?: (value: number) => void
}

const BUTTON_W = 31
const BUTTON_H = 16.5

export const SpinBox: Component<SpinBoxProps> = (props) => {
  const theme = useTheme()
  const [upHovered, setUpHovered] = createSignal(false)
  const [upPressed, setUpPressed] = createSignal(false)
  const [downHovered, setDownHovered] = createSignal(false)
  const [downPressed, setDownPressed] = createSignal(false)

  const min = () => props.min ?? -Infinity
  const max = () => props.max ?? Infinity
  const step = () => props.step ?? 1
  const value = () => props.value ?? 0
  const totalWidth = () => props.width ?? 200
  const editHeight = () => 33

  const clamp = (v: number) => Math.max(min(), Math.min(max(), v))

  const increment = () => {
    if (props.disabled) return
    const next = clamp(value() + step())
    if (next !== value()) props.onChange?.(next)
  }

  const decrement = () => {
    if (props.disabled) return
    const next = clamp(value() - step())
    if (next !== value()) props.onChange?.(next)
  }

  const handleTextChange = (text: string) => {
    const parsed = parseFloat(text)
    if (!isNaN(parsed)) {
      const next = clamp(parsed)
      if (next !== value()) props.onChange?.(next)
    }
  }

  const arrowBg = (hovered: boolean, pressed: boolean) => {
    const t = theme()
    if (props.disabled) return t.controlDisabled
    if (pressed) return t.controlPressed
    if (hovered) return t.controlHover
    return "transparent"
  }

  const arrowColor = () => {
    const t = theme()
    if (props.disabled) return t.foregroundDisabled
    return t.foregroundSecondary
  }

  return (
    <group
      width={totalWidth()}
      height={editHeight()}
      flexDirection="row"
    >
      <LineEdit
        value={String(value())}
        disabled={props.disabled}
        width={totalWidth() - BUTTON_W}
        height={editHeight()}
        onChange={handleTextChange}
      />
      {/* Up/Down button column */}
      <group
        width={BUTTON_W}
        height={editHeight()}
        flexDirection="column"
      >
        {/* Up button */}
        <rect
          width={BUTTON_W}
          height={BUTTON_H}
          fill={arrowBg(upHovered(), upPressed())}
          cornerRadius={theme().radiusSm}
          onPointerEnter={() => setUpHovered(true)}
          onPointerLeave={() => { setUpHovered(false); setUpPressed(false) }}
          onPointerDown={() => { if (!props.disabled) setUpPressed(true) }}
          onPointerUp={() => { if (upPressed()) increment(); setUpPressed(false) }}
          onClick={() => {}}
        >
          <path
            position="absolute"
            x={(BUTTON_W - 16) / 2}
            y={(BUTTON_H - 12) / 2}
            d="M 4 8 L 8 4 L 12 8"
            stroke={arrowColor()}
            strokeWidth={1.5}
            fill="transparent"
          />
        </rect>
        {/* Down button */}
        <rect
          width={BUTTON_W}
          height={BUTTON_H}
          fill={arrowBg(downHovered(), downPressed())}
          cornerRadius={theme().radiusSm}
          onPointerEnter={() => setDownHovered(true)}
          onPointerLeave={() => { setDownHovered(false); setDownPressed(false) }}
          onPointerDown={() => { if (!props.disabled) setDownPressed(true) }}
          onPointerUp={() => { if (downPressed()) decrement(); setDownPressed(false) }}
          onClick={() => {}}
        >
          <path
            position="absolute"
            x={(BUTTON_W - 16) / 2}
            y={(BUTTON_H - 12) / 2}
            d="M 4 4 L 8 8 L 12 4"
            stroke={arrowColor()}
            strokeWidth={1.5}
            fill="transparent"
          />
        </rect>
      </group>
    </group>
  )
}
