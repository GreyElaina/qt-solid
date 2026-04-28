import { createSignal, type Component, type JSX } from "solid-js"

import { useTheme } from "../theme.ts"

export interface ButtonProps {
  children?: JSX.Element
  disabled?: boolean
  accent?: boolean
  onClick?: () => void
  width?: number
  height?: number
  flexGrow?: number
}

export const Button: Component<ButtonProps> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)
  const [pressed, setPressed] = createSignal(false)
  const [focused, setFocused] = createSignal(false)

  const bg = () => {
    const t = theme()
    if (props.disabled) return props.accent ? t.accentDisabled : t.controlDisabled
    if (pressed()) return props.accent ? t.accentPressed : t.controlPressed
    if (hovered()) return props.accent ? t.accentHover : t.controlHover
    return props.accent ? t.accentDefault : t.controlDefault
  }

  const fg = () => {
    const t = theme()
    if (props.disabled) return t.foregroundDisabled
    return props.accent ? t.foregroundOnAccent : t.foregroundPrimary
  }

  const borderColor = () => {
    const t = theme()
    if (props.disabled) return t.strokeDisabled
    if (focused()) return t.strokeFocus
    return t.strokeDefault
  }

  return (
    <rect
      fill={bg()}
      stroke={borderColor()}
      strokeWidth={focused() ? theme().focusStrokeWidth : 1}
      cornerRadius={theme().radiusMd}
      flexDirection="column"
      alignItems="center"
      justifyContent="center"
      padding={props.height != null ? 0 : 5}
      width={props.width}
      height={props.height ?? 32}
      flexGrow={props.flexGrow}
      focusable={!props.disabled}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => { setHovered(false); setPressed(false) }}
      onPointerDown={() => { if (!props.disabled) setPressed(true) }}
      onPointerUp={() => {
        if (pressed() && !props.disabled) {
          props.onClick?.()
        }
        setPressed(false)
      }}
      onClick={() => {}}
      onFocusIn={() => setFocused(true)}
      onFocusOut={() => setFocused(false)}
    >
      <text
        text={typeof props.children === "string" ? props.children : ""}
        fontSize={theme().fontSizeBody}
        color={fg()}
      />
    </rect>
  )
}
