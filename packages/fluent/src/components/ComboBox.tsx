import { createSignal, For, Show, type Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface ComboBoxProps {
  items: string[]
  selectedIndex?: number
  placeholder?: string
  disabled?: boolean
  width?: number
  onChange?: (index: number, value: string) => void
}

const COMBO_HEIGHT = 33
const ITEM_HEIGHT = 36
const CHEVRON_SIZE = 16

const ComboBoxItem: Component<{
  text: string
  selected: boolean
  onSelect: () => void
}> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)

  const bg = () => {
    if (props.selected) return theme().accentDefault
    if (hovered()) return theme().controlHover
    return "transparent"
  }

  const fg = () => {
    if (props.selected) return theme().foregroundOnAccent
    return theme().foregroundPrimary
  }

  return (
    <rect
      h={ITEM_HEIGHT}
      row
      align="center left"
      padding={theme().spacingMd}
      fill={bg()}
      cornerRadius={theme().radiusSm}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onPointerUp={props.onSelect}
      onClick={() => {}}
    >
      <text
        text={props.text}
        fontSize={theme().fontSizeBody}
        color={fg()}
      />
    </rect>
  )
}

export const ComboBox: Component<ComboBoxProps> = (props) => {
  const theme = useTheme()
  const [open, setOpen] = createSignal(false)
  const [hovered, setHovered] = createSignal(false)

  const w = () => props.width ?? 200
  const selectedText = () => {
    const idx = props.selectedIndex
    if (idx != null && idx >= 0 && idx < props.items.length) {
      return props.items[idx]
    }
    return ""
  }

  const bg = () => {
    if (props.disabled) return theme().controlDisabled
    if (hovered()) return theme().controlHover
    return theme().controlDefault
  }

  const fg = () => {
    if (props.disabled) return theme().foregroundDisabled
    if (selectedText()) return theme().foregroundPrimary
    return theme().foregroundDisabled
  }

  return (
    <group w={w()}>
      <rect
        w={w()}
        h={COMBO_HEIGHT}
        row
        align="center left"
        fill={bg()}
        stroke={theme().strokeDefault}
        strokeWidth={1}
        cornerRadius={theme().radiusMd}
        onPointerEnter={() => setHovered(true)}
        onPointerLeave={() => setHovered(false)}
        onPointerUp={() => { if (!props.disabled) setOpen((v) => !v) }}
        onClick={() => {}}
      >
        <text
          absolute
          transformX={theme().spacingMd}
          transformY={(COMBO_HEIGHT - theme().fontSizeBody) / 2}
          text={selectedText() || props.placeholder || ""}
          fontSize={theme().fontSizeBody}
          color={fg()}
        />
        <path
          absolute
          transformX={w() - CHEVRON_SIZE - theme().spacingMd}
          transformY={(COMBO_HEIGHT - CHEVRON_SIZE) / 2}
          d="M 4 6 L 8 10 L 12 6"
          stroke={props.disabled ? theme().foregroundDisabled : theme().foregroundSecondary}
          strokeWidth={1.5}
          fill="transparent"
        />
      </rect>
      <Show when={open()}>
        <rect
          absolute
          transformY={COMBO_HEIGHT + 2}
          w={w()}
          padding={theme().spacingSm}
          fill={theme().backgroundDefault}
          stroke={theme().strokeDefault}
          strokeWidth={1}
          cornerRadius={theme().radiusMd}
        >
          <For each={props.items}>
            {(item, index) => (
              <ComboBoxItem
                text={item}
                selected={index() === (props.selectedIndex ?? -1)}
                onSelect={() => {
                  if (index() !== (props.selectedIndex ?? -1)) {
                    props.onChange?.(index(), item)
                  }
                  setOpen(false)
                }}
              />
            )}
          </For>
        </rect>
      </Show>
    </group>
  )
}
