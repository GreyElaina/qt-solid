import { createSignal, For, Show, type Component, type JSX } from "solid-js"

import { ScrollView } from "@qt-solid/solid"
import type { SizingValue } from "@qt-solid/solid"
import { useTheme } from "../theme.ts"

export interface NavItem {
  key: string
  text: string
  icon?: string
}

export interface NavigationViewProps {
  items: NavItem[]
  selectedKey?: string
  onSelect?: (key: string) => void
  width?: number
  height?: number
  w?: SizingValue
  h?: SizingValue
  header?: JSX.Element
  footer?: JSX.Element
}

export const NavigationView: Component<NavigationViewProps> = (props) => {
  const theme = useTheme()
  const [hoveredKey, setHoveredKey] = createSignal<string | null>(null)
  const [pressedKey, setPressedKey] = createSignal<string | null>(null)

  const width = () => props.width ?? 280
  const pad = 4
  const itemW = () => width() - pad * 2

  const isSelected = (key: string) => key === props.selectedKey

  const itemBg = (key: string) => {
    const t = theme()
    if (isSelected(key)) return t.accentDefault
    if (key === hoveredKey()) return t.controlHover
    return "transparent"
  }

  const itemFg = (key: string) => {
    const t = theme()
    if (isSelected(key)) return t.foregroundOnAccent
    return t.foregroundPrimary
  }

  const select = (key: string) => {
    if (key !== props.selectedKey) {
      props.onSelect?.(key)
    }
  }

  return (
    <group
      w={props.w ?? width()}
      h={props.h ?? props.height}
      onPointerLeave={() => { setHoveredKey(null); setPressedKey(null) }}
    >
      <Show when={props.header}>{props.header}</Show>

      <ScrollView direction="vertical" w={width()} h="fill">
        <group gap={2} padding={pad}>
          <For each={props.items}>
            {(item) => (
              <rect
                fill={itemBg(item.key)}
                cornerRadius={theme().radiusMd}
                row
                align="center left"
                gap={theme().spacingMd}
                h={40}
                w={itemW()}
                padding={theme().spacingMd}
                onPointerEnter={() => setHoveredKey(item.key)}
                onPointerLeave={() => {
                  if (hoveredKey() === item.key) setHoveredKey(null)
                  if (pressedKey() === item.key) setPressedKey(null)
                }}
                onPointerDown={() => setPressedKey(item.key)}
                onPointerUp={() => {
                  if (pressedKey() === item.key) select(item.key)
                  if (pressedKey() === item.key) setPressedKey(null)
                }}
                onClick={() => {}}
              >
                <Show when={isSelected(item.key)}>
                  <rect
                    transformX={0}
                    transformY={12}
                    w={3}
                    h={16}
                    fill={theme().foregroundOnAccent}
                    cornerRadius={theme().radiusCircular}
                  />
                </Show>

                <group w={24} h={16} align="center">
                  <Show when={item.icon}>
                    <path d={item.icon!} stroke={itemFg(item.key)} strokeWidth={1.2} w={16} h={16} />
                  </Show>
                </group>

                <text
                  text={item.text}
                  fontSize={theme().fontSizeBody}
                  color={itemFg(item.key)}
                />
              </rect>
            )}
          </For>
        </group>
      </ScrollView>

      <Show when={props.footer}>{props.footer}</Show>
    </group>
  )
}
