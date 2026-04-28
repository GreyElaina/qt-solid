import { createSignal, For, type Component, type JSX } from "solid-js"
import { createPopup } from "@qt-solid/solid"
import type { PopupDismissEvent } from "@qt-solid/solid"

import { useTheme } from "../theme.ts"

export interface MenuItem {
  text: string
  icon?: string
  disabled?: boolean
  onClick?: () => void
}

export interface MenuProps {
  items: MenuItem[]
  anchor?: { readonly id: number }
  visible?: boolean
  placement?: "bottom" | "top" | "right" | "left"
  onDismiss?: () => void
  width?: number
}

const ITEM_HEIGHT = 36

const MenuItemRow: Component<{
  item: MenuItem
  onSelect: () => void
}> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)

  const bg = () => {
    if (props.item.disabled) return "transparent"
    if (hovered()) return theme().controlHover
    return "transparent"
  }

  const fg = () => {
    if (props.item.disabled) return theme().foregroundDisabled
    return theme().foregroundPrimary
  }

  return (
    <rect
      fill={bg()}
      cornerRadius={theme().radiusSm}
      height={ITEM_HEIGHT}
      flexDirection="row"
      alignItems="center"
      padding={theme().spacingMd}
      gap={theme().spacingMd}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onPointerUp={() => {
        if (!props.item.disabled) {
          props.item.onClick?.()
          props.onSelect()
        }
      }}
      onClick={() => {}}
    >
      {props.item.icon && (
        <path
          d={props.item.icon}
          fill={fg()}
          width={16}
          height={16}
        />
      )}
      <text
        text={props.item.text}
        fontSize={theme().fontSizeBody}
        color={fg()}
      />
    </rect>
  )
}

export const Menu: Component<MenuProps> = (props) => {
  const theme = useTheme()
  const menuWidth = () => props.width ?? 200

  const popup = createPopup(
    () => ({
      visible: props.visible ?? false,
      anchor: props.anchor,
      placement: props.placement ?? "bottom",
      width: menuWidth(),
      onDismiss: (_e: PopupDismissEvent) => props.onDismiss?.(),
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
        <For each={props.items}>
          {(item) => (
            <MenuItemRow
              item={item}
              onSelect={() => props.onDismiss?.()}
            />
          )}
        </For>
      </rect>
    ),
  )

  return popup.render()
}
