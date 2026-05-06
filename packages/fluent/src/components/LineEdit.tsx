import { createSignal, Show, type Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface LineEditProps {
  value?: string
  placeholder?: string
  disabled?: boolean
  error?: boolean
  width?: number
  height?: number
  onChange?: (value: string) => void
  onSubmit?: () => void
}

export const LineEdit: Component<LineEditProps> = (props) => {
  const theme = useTheme()
  const [focused, setFocused] = createSignal(false)

  const w = () => props.width ?? 200
  const h = () => props.height ?? 33
  const val = () => props.value ?? ""

  const bg = () => {
    const t = theme()
    if (props.disabled) return t.controlDisabled
    return t.controlDefault
  }

  const borderColor = () => {
    const t = theme()
    if (props.disabled) return t.strokeDisabled
    return t.strokeDefault
  }

  const bottomAccent = () => {
    if (props.error) return "#C42B1C"
    if (focused() && !props.disabled) return theme().accentDefault
    return undefined
  }

  const textColor = () => {
    const t = theme()
    if (props.disabled) return t.foregroundDisabled
    return t.foregroundPrimary
  }

  const handleTextChange = (info: { text: string; cursor: number; selStart: number; selEnd: number }) => {
    if (info.text !== val()) {
      props.onChange?.(info.text)
    }
  }

  const handleKeyDown = (e: unknown) => {
    const key = (e as { key: string }).key
    if (key === "Return" || key === "Enter") {
      props.onSubmit?.()
    }
  }

  return (
    <rect
      w={w()}
      h={h()}
      fill={bg()}
      stroke={borderColor()}
      strokeWidth={1}
      cornerRadius={theme().radiusMd}
      focusable={!props.disabled}
      onFocusIn={() => setFocused(true)}
      onFocusOut={() => setFocused(false)}
      onKeyDown={handleKeyDown}
    >
      {/* Placeholder text */}
      <Show when={val() === "" && !focused()}>
        <text
          absolute
          transformX={theme().spacingMd}
          transformY={(h() - theme().fontSizeBody) / 2}
          text={props.placeholder ?? ""}
          fontSize={theme().fontSizeBody}
          color={theme().foregroundDisabled}
        />
      </Show>
      {/* Text input */}
      <textinput
        absolute
        transformX={theme().spacingMd}
        transformY={(h() - theme().fontSizeBody) / 2}
        w={w() - theme().spacingMd * 2}
        h={theme().fontSizeBody + 4}
        text={val()}
        fontSize={theme().fontSizeBody}
        color={textColor()}
        onTextChange={handleTextChange}
      />
      {/* Bottom accent bar */}
      <Show when={bottomAccent() !== undefined}>
        <rect
          absolute
          transformX={0}
          transformY={h() - 2}
          w={w()}
          h={2}
          fill={bottomAccent()!}
          cornerRadius={1}
        />
      </Show>
    </rect>
  )
}
