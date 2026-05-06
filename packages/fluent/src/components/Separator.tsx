import type { Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface SeparatorProps {
  length?: number
}

export const HorizontalSeparator: Component<SeparatorProps> = (props) => {
  const theme = useTheme()

  return (
    <rect
      w={props.length}
      h={1}
      fill={theme().strokeDefault}
    />
  )
}

export const VerticalSeparator: Component<SeparatorProps> = (props) => {
  const theme = useTheme()

  return (
    <rect
      w={1}
      h={props.length}
      fill={theme().strokeDefault}
    />
  )
}
