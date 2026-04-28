import { createSignal, type Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface RadioButtonProps {
  checked?: boolean
  disabled?: boolean
  label?: string
  onChange?: (checked: boolean) => void
}

const OUTER_R = 10
const SIZE = OUTER_R * 2

export const RadioButton: Component<RadioButtonProps> = (props) => {
  const theme = useTheme()
  const [hovered, setHovered] = createSignal(false)
  const [pressed, setPressed] = createSignal(false)

  const checked = () => props.checked ?? false

  const outerFill = () => {
    const t = theme()
    if (props.disabled) return checked() ? t.accentDisabled : "transparent"
    if (pressed()) return checked() ? t.accentPressed : "transparent"
    if (hovered()) return checked() ? t.accentHover : "transparent"
    return checked() ? t.accentDefault : "transparent"
  }

  const outerStroke = () => {
    const t = theme()
    if (props.disabled) return t.strokeDisabled
    if (checked()) return outerFill()
    if (hovered()) return t.foregroundSecondary
    return t.strokeDefault
  }

  const innerR = () => {
    if (pressed()) return 4
    if (hovered()) return 6.5
    return 5
  }

  const innerFill = () => {
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
      <group width={SIZE} height={SIZE}>
        <circle
          position="absolute"
          cx={OUTER_R}
          cy={OUTER_R}
          r={OUTER_R}
          fill={outerFill()}
          stroke={outerStroke()}
          strokeWidth={1}
        />
        {checked() && (
          <circle
            position="absolute"
            cx={OUTER_R}
            cy={OUTER_R}
            r={innerR()}
            fill={innerFill()}
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
