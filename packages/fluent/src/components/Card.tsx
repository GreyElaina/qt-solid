import { createSignal, type Component, type JSX } from "solid-js"

import {
  createVariants,
  motion,
  defineIntrinsicComponent,
  type CanvasRectProps,
} from "@qt-solid/solid"
import { useTheme } from "../theme.ts"

export interface CardProps {
  children?: JSX.Element
  clickable?: boolean
  disabled?: boolean
  width?: number
  height?: number
  padding?: number
  borderRadius?: number
  onClick?: () => void
}

const PressableRect = createVariants(
  motion(defineIntrinsicComponent<CanvasRectProps>("rect")),
  {
    base: { scale: 1 },
    variants: {
      interaction: {
        idle: { scale: 1 },
        pressed: { scale: 0.98 },
      },
    },
    defaultVariants: { interaction: "idle" },
    transition: { type: "tween", duration: 0.06, ease: "ease-out" },
  },
)

export const Card: Component<CardProps> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)
  const [pressed, setPressed] = createSignal(false)

  const bgFill = () => {
    const t = theme()
    if (props.disabled) return t.controlDisabled
    if (pressed() && props.clickable) return t.controlPressed
    if (hovered() && props.clickable) return t.controlHover
    return t.backgroundSecondary
  }

  const radius = () => props.borderRadius ?? theme().radiusLg
  const pad = () => props.padding ?? theme().spacingLg

  const interaction = () => (pressed() && props.clickable ? "pressed" : "idle") as "idle" | "pressed"

  return (
    <PressableRect
      interaction={interaction()}
      fill={bgFill()}
      stroke={props.disabled ? theme().strokeDisabled : theme().strokeDefault}
      strokeWidth={1}
      cornerRadius={radius()}
      flexDirection="column"
      padding={pad()}
      width={props.width}
      height={props.height}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => { setHovered(false); setPressed(false) }}
      onPointerDown={() => { if (props.clickable && !props.disabled) setPressed(true) }}
      onPointerUp={() => {
        if (pressed() && props.clickable && !props.disabled) {
          props.onClick?.()
        }
        setPressed(false)
      }}
      onClick={() => {}}
    >
      {props.children}
    </PressableRect>
  )
}
