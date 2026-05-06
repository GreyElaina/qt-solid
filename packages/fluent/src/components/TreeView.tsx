import { createSignal, createMemo, type JSX } from "solid-js"
import { For, Show } from "solid-js"
import { ScrollView } from "@qt-solid/solid"

import { useTheme } from "../theme.ts"

export interface TreeNode<T = unknown> {
  data: T
  children?: TreeNode<T>[]
}

export interface TreeViewProps<T> {
  nodes: TreeNode<T>[]
  renderNode: (data: T, depth: number) => JSX.Element
  width?: number
  height?: number
  itemHeight?: number
  defaultExpanded?: boolean
}

interface FlatRow<T> {
  node: TreeNode<T>
  depth: number
  key: string
  hasChildren: boolean
}

const DEFAULT_ITEM_HEIGHT = 36
const CHEVRON_SIZE = 16
const INDENT_PX = 16

export function TreeView<T>(props: TreeViewProps<T>): JSX.Element {
  const theme = useTheme()
  const [hoveredIndex, setHoveredIndex] = createSignal(-1)
  const [expandedKeys, setExpandedKeys] = createSignal<Set<string>>(new Set())

  const isExpanded = (key: string): boolean => {
    if (expandedKeys().has(key)) return true
    return props.defaultExpanded ?? false
  }

  const toggleExpanded = (key: string) => {
    setExpandedKeys((prev) => {
      const next = new Set(prev)
      if (isExpanded(key)) {
        next.delete(key)
        // When defaultExpanded, we track explicitly collapsed keys by adding them
        // Actually, let's use a simpler model: the set tracks toggled state
        if (props.defaultExpanded) next.add(key)
      } else {
        if (props.defaultExpanded) {
          next.delete(key)
        } else {
          next.add(key)
        }
      }
      return next
    })
  }

  const flatRows = createMemo((): FlatRow<T>[] => {
    const rows: FlatRow<T>[] = []
    const walk = (nodes: TreeNode<T>[], depth: number, prefix: string) => {
      for (let i = 0; i < nodes.length; i++) {
        const node = nodes[i]!
        const key = `${prefix}/${i}`
        const hasChildren = (node.children?.length ?? 0) > 0
        rows.push({ node, depth, key, hasChildren })
        if (hasChildren && isExpanded(key)) {
          walk(node.children!, depth + 1, key)
        }
      }
    }
    walk(props.nodes, 0, "")
    return rows
  })

  const itemH = () => props.itemHeight ?? DEFAULT_ITEM_HEIGHT

  const rowBg = (index: number) => {
    const t = theme()
    if (hoveredIndex() === index) return t.controlHover
    return "transparent"
  }

  // Collapsed: right-pointing chevron
  const chevronCollapsed = "M 4 2 L 10 8 L 4 14"
  // Expanded: down-pointing chevron
  const chevronExpanded = "M 2 4 L 8 10 L 14 4"

  return (
    <ScrollView w={props.width} h={props.height} direction="vertical">
      <For each={flatRows()}>
        {(row, i) => (
          <rect
            fill={rowBg(i())}
            cornerRadius={theme().radiusMd}
            w={props.width}
            h={itemH()}
            row
            align="center left"
            onPointerEnter={() => setHoveredIndex(i())}
            onPointerLeave={() => { if (hoveredIndex() === i()) setHoveredIndex(-1) }}
          >
            {/* Indent spacer */}
            <group w={row.depth * INDENT_PX} h={itemH()} />
            {/* Chevron or spacer */}
            <Show
              when={row.hasChildren}
              fallback={<group w={CHEVRON_SIZE} h={CHEVRON_SIZE} />}
            >
              <group
                w={CHEVRON_SIZE}
                h={CHEVRON_SIZE}
                onClick={() => toggleExpanded(row.key)}
              >
                <path
                  d={isExpanded(row.key) ? chevronExpanded : chevronCollapsed}
                  fill={theme().foregroundSecondary}
                />
              </group>
            </Show>
            {/* Node content */}
            <group w="fill" h={itemH()} align="center left" row>
              {props.renderNode(row.node.data, row.depth)}
            </group>
          </rect>
        )}
      </For>
    </ScrollView>
  )
}
