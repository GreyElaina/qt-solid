import { createSignal, type Component, type JSX } from "solid-js"

import { useTheme } from "../theme.ts"

export interface HyperlinkButtonProps {
  children?: JSX.Element
  disabled?: boolean
  onClick?: () => void
  width?: number
  height?: number
}

export const HyperlinkButton: Component<HyperlinkButtonProps> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)
  const [pressed, setPressed] = createSignal(false)

  const fg = () => {
    const t = theme()
    if (props.disabled) return t.foregroundDisabled
    if (pressed()) return t.accentPressed
    if (hovered()) return t.accentHover
    return t.accentDefault
  }

  return (
    <group
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
    >
      <text
        text={typeof props.children === "string" ? props.children : ""}
        fontSize={theme().fontSizeBody}
        color={fg()}
      />
    </group>
  )
}
