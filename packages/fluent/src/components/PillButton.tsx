import { createSignal, type Component, type JSX } from "solid-js"

import { createVariants, motion, defineIntrinsicComponent, type CanvasRectProps } from "@qt-solid/solid"
import { useTheme } from "../theme.ts"

export interface PillButtonProps {
  children?: JSX.Element
  disabled?: boolean
  accent?: boolean
  onClick?: () => void
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

export const PillButton: Component<PillButtonProps> = (props) => {
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
    <PressableRect
      interaction={pressed() ? "pressed" : "idle"}
      fill={bg()}
      stroke={borderColor()}
      strokeWidth={focused() ? theme().focusStrokeWidth : 1}
      cornerRadius={theme().radiusCircular}
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
    </PressableRect>
  )
}
