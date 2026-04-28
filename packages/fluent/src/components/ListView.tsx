import { createSignal, type JSX } from "solid-js"
import { For } from "solid-js"
import { ScrollView } from "@qt-solid/solid"

import { useTheme } from "../theme.ts"

export interface ListViewProps<T> {
  items: readonly T[]
  renderItem: (item: T, index: number) => JSX.Element
  width?: number
  height?: number
  itemHeight?: number
  selectedIndex?: number
  onSelect?: (index: number) => void
}

const DEFAULT_ITEM_HEIGHT = 36

export function ListView<T>(props: ListViewProps<T>): JSX.Element {
  const theme = useTheme()
  const [hoveredIndex, setHoveredIndex] = createSignal(-1)

  const itemH = () => props.itemHeight ?? DEFAULT_ITEM_HEIGHT

  const rowBg = (index: number) => {
    const t = theme()
    if (props.selectedIndex === index) return t.accentDefault
    if (hoveredIndex() === index) return t.controlHover
    return "transparent"
  }

  const rowFg = (index: number) => {
    const t = theme()
    if (props.selectedIndex === index) return t.foregroundOnAccent
    return t.foregroundPrimary
  }

  return (
    <ScrollView width={props.width} height={props.height} direction="vertical">
      <For each={props.items}>
        {(item, i) => (
          <rect
            fill={rowBg(i())}
            cornerRadius={theme().radiusMd}
            width={props.width}
            height={itemH()}
            flexDirection="row"
            alignItems="center"
            padding={theme().spacingMd}
            onPointerEnter={() => setHoveredIndex(i())}
            onPointerLeave={() => { if (hoveredIndex() === i()) setHoveredIndex(-1) }}
            onClick={() => props.onSelect?.(i())}
          >
            {props.renderItem(item, i())}
          </rect>
        )}
      </For>
    </ScrollView>
  )
}
