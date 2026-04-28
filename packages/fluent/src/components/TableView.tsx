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
    <group width={props.width} flexDirection="column">
      {/* Header */}
      <rect fill={theme().backgroundSecondary} width={props.width} height={headerH()} flexDirection="row" alignItems="center">
        <For each={props.columns}>
          {(col) => (
            <group width={col.width} height={headerH()} alignItems="center" justifyContent="center">
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
      <rect width={props.width} height={1} fill={theme().strokeDefault} />
      {/* Body */}
      <ScrollView width={props.width} height={bodyHeight()} direction="vertical">
        <For each={props.items}>
          {(item, i) => (
            <rect
              fill={rowBg(i())}
              cornerRadius={theme().radiusMd}
              width={props.width}
              height={rowH()}
              flexDirection="row"
              alignItems="center"
              onPointerEnter={() => setHoveredIndex(i())}
              onPointerLeave={() => { if (hoveredIndex() === i()) setHoveredIndex(-1) }}
              onClick={() => props.onSelect?.(i())}
            >
              <For each={props.columns}>
                {(col) => (
                  <group width={col.width} height={rowH()} alignItems="center" justifyContent="center">
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
