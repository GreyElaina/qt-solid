import type { Component } from "solid-js"

import { useTheme } from "../theme.ts"
import { FluentIcons, type FluentIconName } from "./fluent-icons.ts"

export interface IconProps {
  name: FluentIconName
  size?: number
  color?: string
}

export const Icon: Component<IconProps> = (props) => {
  const theme = useTheme()
  const icon = () => FluentIcons[props.name]
  const s = () => props.size ?? 16
  const c = () => props.color ?? theme().foregroundPrimary

  return (
    <group width={s()} height={s()}>
      <path
        d={icon().d}
        stroke={c()}
        strokeWidth={1.2}
        width={s()}
        height={s()}
      />
    </group>
  )
}
