import { createSignal, For, type Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface TabItem {
  key: string
  text: string
  closable?: boolean
}

export interface TabBarProps {
  items: TabItem[]
  selectedKey?: string
  onSelect?: (key: string) => void
  onClose?: (key: string) => void
}

export const TabBar: Component<TabBarProps> = (props) => {
  const theme = useTheme()
  const [hoveredKey, setHoveredKey] = createSignal<string | null>(null)

  const isSelected = (key: string) => key === props.selectedKey

  const tabBg = (key: string) => {
    const t = theme()
    if (isSelected(key)) return t.backgroundDefault
    if (key === hoveredKey()) return t.controlHover
    return "transparent"
  }

  const tabFg = (key: string) => {
    const t = theme()
    if (isSelected(key)) return t.foregroundPrimary
    return t.foregroundSecondary
  }

  return (
    <group flexDirection="column">
      <group flexDirection="row" alignItems="flex-end">
        <For each={props.items}>
          {(item) => (
            <rect
              fill={tabBg(item.key)}
              cornerRadius={isSelected(item.key)
                ? { topLeft: theme().radiusMd, topRight: theme().radiusMd, bottomRight: 0, bottomLeft: 0 }
                : 0}
              height={36}
              padding={theme().spacingMd}
              flexDirection="row"
              alignItems="center"
              gap={theme().spacingSm}
              onPointerEnter={() => setHoveredKey(item.key)}
              onPointerLeave={() => setHoveredKey(null)}
              onPointerUp={() => props.onSelect?.(item.key)}
              onClick={() => {}}
            >
              <text
                text={item.text}
                fontSize={theme().fontSizeBody}
                color={tabFg(item.key)}
              />

              {item.closable && (
                <group
                  width={16}
                  height={16}
                  alignItems="center"
                  justifyContent="center"
                  onPointerUp={() => props.onClose?.(item.key)}
                  onClick={() => {}}
                >
                  <text text="×" fontSize={12} color={tabFg(item.key)} />
                </group>
              )}
            </rect>
          )}
        </For>
      </group>

      <rect height={1} fill={theme().strokeDefault} />
    </group>
  )
}
