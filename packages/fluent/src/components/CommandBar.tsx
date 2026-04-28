import { createSignal, For, Show, type Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface CommandBarItem {
  key: string
  text?: string
  icon?: string
  disabled?: boolean
  onClick?: () => void
}

export interface CommandBarProps {
  items: CommandBarItem[]
  width?: number
}

export const CommandBar: Component<CommandBarProps> = (props) => {
  const theme = useTheme()
  const [hoveredKey, setHoveredKey] = createSignal<string | null>(null)

  return (
    <group flexDirection="row" alignItems="center" gap={theme().spacingXs} width={props.width}>
      <For each={props.items}>
        {(item) => {
          if (item.key === "separator") {
            return <rect width={1} height={20} fill={theme().strokeDefault} />
          }

          const isHovered = () => hoveredKey() === item.key && !item.disabled

          return (
            <rect
              fill={isHovered() ? theme().controlHover : "transparent"}
              cornerRadius={theme().radiusMd}
              flexDirection="row"
              alignItems="center"
              gap={theme().spacingSm}
              height={32}
              padding={theme().spacingMd}
              onPointerEnter={() => setHoveredKey(item.key)}
              onPointerLeave={() => setHoveredKey(null)}
              onPointerUp={() => { if (!item.disabled) item.onClick?.() }}
            >
              <Show when={item.icon}>
                <path
                  d={item.icon!}
                  stroke={item.disabled ? theme().foregroundDisabled : theme().foregroundPrimary}
                  strokeWidth={1.2}
                  width={16}
                  height={16}
                />
              </Show>

              <Show when={item.text}>
                <text
                  text={item.text!}
                  fontSize={theme().fontSizeBody}
                  color={item.disabled ? theme().foregroundDisabled : theme().foregroundPrimary}
                />
              </Show>
            </rect>
          )
        }}
      </For>
    </group>
  )
}
