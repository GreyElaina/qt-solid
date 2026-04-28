import { createSignal, For, Show, type Component, type JSX } from "solid-js"

import { ScrollView } from "@qt-solid/solid"
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
  flexGrow?: number
  flexShrink?: number
  header?: JSX.Element
  footer?: JSX.Element
}

export const NavigationView: Component<NavigationViewProps> = (props) => {
  const theme = useTheme()
  const [hoveredKey, setHoveredKey] = createSignal<string | null>(null)
  const [pressedKey, setPressedKey] = createSignal<string | null>(null)

  const w = () => props.width ?? 280
  const pad = 4
  const itemW = () => w() - pad * 2

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
      flexDirection="column"
      width={w()}
      height={props.height}
      flexGrow={props.flexGrow ?? 0}
      flexShrink={props.flexShrink ?? 0}
      onPointerLeave={() => { setHoveredKey(null); setPressedKey(null) }}
    >
      <Show when={props.header}>{props.header}</Show>

      <ScrollView direction="vertical" width={w()} flexGrow={1}>
        <group flexDirection="column" gap={2} padding={pad}>
          <For each={props.items}>
            {(item) => (
              <rect
                fill={itemBg(item.key)}
                cornerRadius={theme().radiusMd}
                flexDirection="row"
                alignItems="center"
                gap={theme().spacingMd}
                height={40}
                width={itemW()}
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
                    x={0}
                    y={12}
                    width={3}
                    height={16}
                    fill={theme().foregroundOnAccent}
                    cornerRadius={theme().radiusCircular}
                  />
                </Show>

                <group width={24} height={16} alignItems="center" justifyContent="center">
                  <Show when={item.icon}>
                    <path d={item.icon!} stroke={itemFg(item.key)} strokeWidth={1.2} width={16} height={16} />
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
