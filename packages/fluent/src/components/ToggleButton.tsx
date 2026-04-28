import { createSignal, type Component, type JSX } from "solid-js"

import { createVariants, motion, defineIntrinsicComponent, type CanvasRectProps } from "@qt-solid/solid"
import { useTheme } from "../theme.ts"

export interface ToggleButtonProps {
  children?: JSX.Element
  checked?: boolean
  disabled?: boolean
  onClick?: () => void
  onChange?: (checked: boolean) => void
  width?: number
  height?: number
}

const PressableRect = createVariants(motion(defineIntrinsicComponent<CanvasRectProps>("rect")), {
  base: { scale: 1, opacity: 1 },
  variants: {
    interaction: {
      idle: { scale: 1, opacity: 1 },
      pressed: { scale: 0.98, opacity: 0.9 },
    },
  },
  defaultVariants: { interaction: "idle" },
  transition: { type: "tween", duration: 0.06, ease: "ease-out" },
})

export const ToggleButton: Component<ToggleButtonProps> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)
  const [pressed, setPressed] = createSignal(false)
  const [focused, setFocused] = createSignal(false)

  const checked = () => props.checked ?? false

  const bg = () => {
    const t = theme()
    const accent = checked()
    if (props.disabled) return accent ? t.accentDisabled : t.controlDisabled
    if (pressed()) return accent ? t.accentPressed : t.controlPressed
    if (hovered()) return accent ? t.accentHover : t.controlHover
    return accent ? t.accentDefault : t.controlDefault
  }

  const fg = () => {
    const t = theme()
    if (props.disabled) return t.foregroundDisabled
    return checked() ? t.foregroundOnAccent : t.foregroundPrimary
  }

  const borderColor = () => {
    const t = theme()
    if (props.disabled) return t.strokeDisabled
    if (focused()) return t.strokeFocus
    return t.strokeDefault
  }

  const toggle = () => {
    if (!props.disabled) {
      props.onClick?.()
      props.onChange?.(!checked())
    }
  }

  return (
    <PressableRect
      interaction={pressed() ? "pressed" : "idle"}
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
      focusable={!props.disabled}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => { setHovered(false); setPressed(false) }}
      onPointerDown={() => { if (!props.disabled) setPressed(true) }}
      onPointerUp={() => {
        if (pressed()) toggle()
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
    </PressableRect>
  )
}
