import { createSignal, For, type Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface PivotItem {
  key: string
  text: string
}

export interface PivotProps {
  items: PivotItem[]
  selectedKey?: string
  onSelect?: (key: string) => void
  width?: number
}

export const Pivot: Component<PivotProps> = (props) => {
  const theme = useTheme()
  const [hoveredKey, setHoveredKey] = createSignal<string | null>(null)

  const fg = (key: string) => {
    const t = theme()
    if (key === props.selectedKey) return t.accentDefault
    if (key === hoveredKey()) return t.foregroundPrimary
    return t.foregroundSecondary
  }

  return (
    <group row align="bottom left" gap={theme().spacingXl} w={props.width}>
      <For each={props.items}>
        {(item) => (
          <group
            align="top center"
            gap={theme().spacingSm}
            onPointerEnter={() => setHoveredKey(item.key)}
            onPointerLeave={() => setHoveredKey(null)}
            onPointerUp={() => props.onSelect?.(item.key)}
          >
            <text
              text={item.text}
              fontSize={theme().fontSizeBody}
              color={fg(item.key)}
            />
            {item.key === props.selectedKey && (
              <rect w={20} h={2} fill={theme().accentDefault} cornerRadius={1} />
            )}
          </group>
        )}
      </For>
    </group>
  )
}
