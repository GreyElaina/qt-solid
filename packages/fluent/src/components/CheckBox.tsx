import { createSignal, type Component } from "solid-js"

import { useTheme } from "../theme.ts"

// Fluent-style checkmark SVG path (12x12 viewBox, scaled to box size)
const CHECK_PATH = "M 2.5 6 L 5 8.5 L 9.5 3.5"

export interface CheckBoxProps {
  checked?: boolean
  disabled?: boolean
  label?: string
  onChange?: (checked: boolean) => void
}

const BOX_SIZE = 18
const ICON_OFFSET = 3

export const CheckBox: Component<CheckBoxProps> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)
  const [pressed, setPressed] = createSignal(false)

  const checked = () => props.checked ?? false

  const boxBg = () => {
    const t = theme()
    if (props.disabled) return checked() ? t.accentDisabled : t.controlDisabled
    if (pressed()) return checked() ? t.accentPressed : t.controlPressed
    if (hovered()) return checked() ? t.accentHover : t.controlHover
    return checked() ? t.accentDefault : t.controlDefault
  }

  const boxBorder = () => {
    const t = theme()
    if (props.disabled) return t.strokeDisabled
    if (checked()) return boxBg()
    if (hovered()) return t.foregroundSecondary
    return t.strokeDefault
  }

  const checkColor = () => {
    const t = theme()
    if (props.disabled) return t.foregroundDisabled
    return t.foregroundOnAccent
  }

  const toggle = () => {
    if (!props.disabled) {
      props.onChange?.(!checked())
    }
  }

  return (
    <group
      flexDirection="row"
      alignItems="center"
      gap={theme().spacingMd}
      focusable={!props.disabled}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => { setHovered(false); setPressed(false) }}
      onPointerDown={() => { if (!props.disabled) setPressed(true) }}
      onPointerUp={() => {
        if (pressed()) toggle()
        setPressed(false)
      }}
      onClick={() => {}}
    >
      <group width={BOX_SIZE} height={BOX_SIZE}>
        <rect
          position="absolute"
          width={BOX_SIZE}
          height={BOX_SIZE}
          fill={boxBg()}
          stroke={boxBorder()}
          strokeWidth={1}
          cornerRadius={theme().radiusSm}
        />
        {checked() && (
          <path
            position="absolute"
            d={CHECK_PATH}
            x={ICON_OFFSET}
            y={ICON_OFFSET}
            stroke={checkColor()}
            strokeWidth={1.5}
            width={12}
            height={12}
          />
        )}
      </group>
      {props.label && (
        <text
          text={props.label}
          fontSize={theme().fontSizeBody}
          color={props.disabled ? theme().foregroundDisabled : theme().foregroundPrimary}
        />
      )}
    </group>
  )
}
