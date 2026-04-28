import { createSignal, type Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface ToggleProps {
  checked?: boolean
  disabled?: boolean
  onChange?: (checked: boolean) => void
}

const TRACK_W = 40
const TRACK_H = 20
const THUMB_R = 6
const THUMB_MARGIN = 4

export const Toggle: Component<ToggleProps> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)
  const [pressed, setPressed] = createSignal(false)

  const checked = () => props.checked ?? false

  const thumbX = () => checked()
    ? TRACK_W - THUMB_MARGIN - THUMB_R
    : THUMB_MARGIN + THUMB_R

  const trackBg = () => {
    const t = theme()
    if (props.disabled) return checked() ? t.accentDisabled : t.controlDisabled
    if (pressed()) return checked() ? t.accentPressed : t.controlPressed
    if (hovered()) return checked() ? t.accentHover : t.controlHover
    return checked() ? t.accentDefault : t.controlDefault
  }

  const trackBorder = () => {
    const t = theme()
    if (props.disabled) return t.strokeDisabled
    if (checked()) return trackBg()
    return t.strokeDefault
  }

  const thumbColor = () => {
    const t = theme()
    if (props.disabled) return t.foregroundDisabled
    return checked() ? t.foregroundOnAccent : t.foregroundPrimary
  }

  const toggle = () => {
    if (!props.disabled) {
      props.onChange?.(!checked())
    }
  }

  return (
    <group
      width={TRACK_W}
      height={TRACK_H}
      focusable={!props.disabled}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => { setHovered(false); setPressed(false) }}
      onPointerDown={() => { if (!props.disabled) setPressed(true) }}
      onPointerUp={() => {
        if (pressed()) toggle()
        setPressed(false)
      }}
      onClick={() => {}}
    >
      <rect
        position="absolute"
        width={TRACK_W}
        height={TRACK_H}
        fill={trackBg()}
        stroke={trackBorder()}
        strokeWidth={1}
        cornerRadius={TRACK_H / 2}
      />
      <circle
        position="absolute"
        cx={thumbX()}
        cy={TRACK_H / 2}
        r={pressed() ? THUMB_R + 1 : THUMB_R}
        fill={thumbColor()}
      />
    </group>
  )
}
