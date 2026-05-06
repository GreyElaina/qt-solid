import { createSignal, type Component, type JSX } from "solid-js"

import { type SizingValue, createVariants, type RectProps } from "@qt-solid/solid"
import { useTheme } from "../theme.ts"

export interface TransparentButtonProps {
  children?: JSX.Element
  disabled?: boolean
  onClick?: () => void
  width?: number
  height?: number
  w?: SizingValue
}

const RectBase: Component<RectProps> = (props) => <rect {...props} />

const PressableRect = createVariants(RectBase, {
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

export const TransparentButton: Component<TransparentButtonProps> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)
  const [pressed, setPressed] = createSignal(false)

  const bg = () => {
    const t = theme()
    if (props.disabled) return "transparent"
    if (pressed()) return t.controlPressed
    if (hovered()) return t.controlHover
    return "transparent"
  }

  const fg = () => {
    const t = theme()
    if (props.disabled) return t.foregroundDisabled
    return t.foregroundPrimary
  }

  return (
    <PressableRect
      interaction={pressed() ? "pressed" : "idle"}
      fill={bg()}
      cornerRadius={theme().radiusMd}
      align="center"
      padding={props.height != null ? 0 : 5}
      w={props.w ?? props.width}
      h={props.height ?? 32}
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
    >
      <text
        text={typeof props.children === "string" ? props.children : ""}
        fontSize={theme().fontSizeBody}
        color={fg()}
      />
    </PressableRect>
  )
}
