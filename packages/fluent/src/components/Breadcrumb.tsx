import { createSignal, For, type Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface BreadcrumbItem {
  key: string
  text: string
}

export interface BreadcrumbProps {
  items: BreadcrumbItem[]
  onSelect?: (key: string) => void
}

export const Breadcrumb: Component<BreadcrumbProps> = (props) => {
  const theme = useTheme()
  const [hoveredKey, setHoveredKey] = createSignal<string | null>(null)

  const isLast = (index: number) => index === props.items.length - 1

  return (
    <group flexDirection="row" alignItems="center" gap={theme().spacingSm}>
      <For each={props.items}>
        {(item, index) => (
          <>
            <group
              onPointerEnter={() => { if (!isLast(index())) setHoveredKey(item.key) }}
              onPointerLeave={() => setHoveredKey(null)}
              onPointerUp={() => { if (!isLast(index())) props.onSelect?.(item.key) }}
            >
              <text
                text={item.text}
                fontSize={theme().fontSizeBody}
                color={
                  isLast(index())
                    ? theme().foregroundPrimary
                    : hoveredKey() === item.key
                      ? theme().accentDefault
                      : theme().foregroundSecondary
                }
              />
            </group>
            {!isLast(index()) && (
              <text text="›" fontSize={theme().fontSizeBody} color={theme().foregroundSecondary} />
            )}
          </>
        )}
      </For>
    </group>
  )
}
