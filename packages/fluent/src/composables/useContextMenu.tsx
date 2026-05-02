import { createSignal, For, Show } from "solid-js"
import type { JSX } from "solid-js"
import { createPopup } from "@qt-solid/solid"
import type { PopupDismissEvent } from "@qt-solid/solid"
import type { MenuItem } from "../components/Menu.tsx"
import { useTheme } from "../theme.ts"
import { MenuItemRow } from "../components/Menu.tsx"

export interface UseContextMenuOptions {
  items: () => MenuItem[]
  width?: number
}

export interface ContextMenuEvent {
  x: number
  y: number
  screenX: number
  screenY: number
}

export interface UseContextMenuResult {
  /** Handler to pass as `onContextMenu` on a canvas fragment. */
  onContextMenu: (e: ContextMenuEvent) => void
  /** Render function — call once in your component's JSX tree. */
  menu: () => JSX.Element
}

export function useContextMenu(options: UseContextMenuOptions): UseContextMenuResult {
  const [visible, setVisible] = createSignal(false)
  const [position, setPosition] = createSignal({ x: 0, y: 0 })
  const menuWidth = () => options.width ?? 200
  const theme = useTheme()

  const dismiss = () => setVisible(false)

  const popup = createPopup(
    () => ({
      visible: visible(),
      screenX: position().x,
      screenY: position().y,
      width: menuWidth(),
      onDismiss: (_e: PopupDismissEvent) => dismiss(),
    }),
    () => (
      <rect
        fill={theme().backgroundDefault}
        stroke={theme().strokeDefault}
        strokeWidth={1}
        cornerRadius={theme().radiusMd}
        width={menuWidth()}
        flexDirection="column"
        padding={theme().spacingSm}
      >
        <For each={options.items()}>
          {(item) => (
            <MenuItemRow item={item} onSelect={dismiss} />
          )}
        </For>
      </rect>
    ),
  )

  const onContextMenu = (e: ContextMenuEvent) => {
    setPosition({ x: e.screenX, y: e.screenY })
    setVisible(true)
  }

  return {
    onContextMenu,
    menu: () => popup.render(),
  }
}
