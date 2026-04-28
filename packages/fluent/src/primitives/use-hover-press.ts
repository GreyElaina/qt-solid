import { createSignal, type Accessor } from "solid-js"

export interface UseHoverPressOptions {
  disabled?: () => boolean
  onPress?: () => void
}

export interface UseHoverPressResult {
  hovered: Accessor<boolean>
  pressed: Accessor<boolean>
  focused: Accessor<boolean>
  handlers: {
    onPointerEnter: () => void
    onPointerLeave: () => void
    onPointerDown: () => void
    onPointerUp: () => void
    onClick: () => void
    onFocusIn: () => void
    onFocusOut: () => void
  }
}

export function useHoverPress(options?: UseHoverPressOptions): UseHoverPressResult {
  const [hovered, setHovered] = createSignal(false)
  const [pressed, setPressed] = createSignal(false)
  const [focused, setFocused] = createSignal(false)
  const isDisabled = () => options?.disabled?.() ?? false

  return {
    hovered,
    pressed,
    focused,
    handlers: {
      onPointerEnter: () => setHovered(true),
      onPointerLeave: () => { setHovered(false); setPressed(false) },
      onPointerDown: () => { if (!isDisabled()) setPressed(true) },
      onPointerUp: () => {
        if (pressed() && !isDisabled()) options?.onPress?.()
        setPressed(false)
      },
      onClick: () => {},
      onFocusIn: () => setFocused(true),
      onFocusOut: () => setFocused(false),
    },
  }
}
