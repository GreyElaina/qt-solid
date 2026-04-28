import { createSignal, Show, type Component, type JSX } from "solid-js"

import { useTheme } from "../theme.ts"

export interface SettingCardProps {
  title: string
  description?: string
  icon?: string
  children?: JSX.Element
  width?: number
  onClick?: () => void
}

export const SettingCard: Component<SettingCardProps> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)

  const bg = () => {
    const t = theme()
    if (props.onClick && hovered()) return t.backgroundTertiary
    return t.backgroundSecondary
  }

  return (
    <rect
      flexDirection="row"
      alignItems="center"
      gap={theme().spacingLg}
      padding={theme().spacingLg}
      width={props.width}
      fill={bg()}
      cornerRadius={theme().radiusLg}
      stroke={theme().strokeDefault}
      strokeWidth={1}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onPointerUp={() => props.onClick?.()}
    >
      <Show when={props.icon}>
        <path d={props.icon!} stroke={theme().foregroundPrimary} strokeWidth={1.2} width={20} height={20} />
      </Show>

      <group flexDirection="column" gap={theme().spacingXs} flexGrow={1}>
        <text text={props.title} fontSize={theme().fontSizeBody} color={theme().foregroundPrimary} />
        <Show when={props.description}>
          <text text={props.description!} fontSize={theme().fontSizeCaption} color={theme().foregroundSecondary} />
        </Show>
      </group>

      <Show when={props.children}>{props.children}</Show>
    </rect>
  )
}
