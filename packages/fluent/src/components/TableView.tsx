import { createSignal, type JSX } from "solid-js"
import { For } from "solid-js"
import { ScrollView } from "@qt-solid/solid"

import { useTheme } from "../theme.ts"

export interface TableColumn<T> {
  key: string
  title: string
  width?: number
  render?: (item: T, index: number) => JSX.Element
}

export interface TableViewProps<T> {
  columns: TableColumn<T>[]
  items: readonly T[]
  width?: number
  height?: number
  headerHeight?: number
  rowHeight?: number
  selectedIndex?: number
  onSelect?: (index: number) => void
}

const DEFAULT_ROW_HEIGHT = 36

export function TableView<T>(props: TableViewProps<T>): JSX.Element {
  const theme = useTheme()
  const [hoveredIndex, setHoveredIndex] = createSignal(-1)

  const headerH = () => props.headerHeight ?? DEFAULT_ROW_HEIGHT
  const rowH = () => props.rowHeight ?? DEFAULT_ROW_HEIGHT
  const bodyHeight = () => (props.height ?? 300) - headerH()

  const rowBg = (index: number) => {
    const t = theme()
    if (props.selectedIndex === index) return t.accentDefault
    if (hoveredIndex() === index) return t.controlHover
    return "transparent"
  }

  const cellContent = (col: TableColumn<T>, item: T, index: number): JSX.Element => {
    if (col.render) return col.render(item, index)
    const value = (item as Record<string, unknown>)[col.key]
    return <text text={String(value ?? "")} fontSize={theme().fontSizeBody} color={theme().foregroundPrimary} />
  }

  return (
    <group w={props.width}>
      {/* Header */}
      <rect fill={theme().backgroundSecondary} w={props.width} h={headerH()} row align="center left">
        <For each={props.columns}>
          {(col) => (
            <group w={col.width} h={headerH()} align="center">
              <text
                text={col.title}
                fontSize={theme().fontSizeBody}
                color={theme().foregroundSecondary}
              />
            </group>
          )}
        </For>
      </rect>
      {/* Header bottom border */}
      <rect w={props.width} h={1} fill={theme().strokeDefault} />
      {/* Body */}
      <ScrollView w={props.width} h={bodyHeight()} direction="vertical">
        <For each={props.items}>
          {(item, i) => (
            <rect
              fill={rowBg(i())}
              cornerRadius={theme().radiusMd}
              w={props.width}
              h={rowH()}
              row
              align="center left"
              onPointerEnter={() => setHoveredIndex(i())}
              onPointerLeave={() => { if (hoveredIndex() === i()) setHoveredIndex(-1) }}
              onClick={() => props.onSelect?.(i())}
            >
              <For each={props.columns}>
                {(col) => (
                  <group w={col.width} h={rowH()} align="center">
                    {cellContent(col, item, i())}
                  </group>
                )}
              </For>
            </rect>
          )}
        </For>
      </ScrollView>
    </group>
  )
}
